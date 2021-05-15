mod work;

use chrono::{DateTime, Utc};
use clap::ArgSettings::HideEnvValues;
use clap::{crate_name, crate_version, Clap};
use log::{debug, info, trace};
use reqwest::header::HeaderMap;
use sqlx::postgres::types::PgInterval;
use sqlx::postgres::PgConnectOptions;
use sqlx::{Connection, PgConnection};
use std::cmp::min;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::delay_for;
use uuid::Uuid;

use work::*;

#[derive(Debug, Clone, Clap)]
#[clap(author, about, version)]
struct Config {
    /// Optional Sentry DSN for error reporting
    #[clap(long, env)]
    sentry_dsn: Option<String>,

    /// Database URL (with credentials)
    #[clap(long, env, setting = HideEnvValues)]
    database_url: String,

    /// Worker ID or name (if empty, will generate a random UUID)
    #[clap(long, env)]
    worker_id: Option<String>,

    /// Worker version (if empty, will use version from Cargo.toml)
    #[clap(long, env)]
    worker_version: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(non_snake_case)]
pub struct RequestAttempt {
    pub request_attempt__id: Uuid,
    pub event__id: Uuid,
    pub subscription__id: Uuid,
    pub created_at: DateTime<Utc>,
    pub retry_count: i16,
    pub http_method: String,
    pub http_url: String,
    pub http_headers: serde_json::Value,
    pub payload: Vec<u8>,
    pub payload_content_type: String,
}

impl RequestAttempt {
    /// Parse headers of HTTP target from JSON and prepare them to be fed to reqwest
    fn headers(&self) -> anyhow::Result<HeaderMap> {
        let hashmap = serde_json::from_value::<HashMap<String, String>>(self.http_headers.clone())?;
        let headermap = HeaderMap::try_from(&hashmap)?;
        Ok(headermap)
    }
}

/// How long to wait when there are no unprocessed items to pick
const POLLING_SLEEP: Duration = Duration::from_secs(1);

/// How long to wait before first retry
const MINIMUM_RETRY_DELAY: Duration = Duration::from_secs(5);

/// How long to wait between retries at maximum
const MAXIMUM_RETRY_DELAY: Duration = Duration::from_secs(5 * 60);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    let worker_id = config
        .worker_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let worker_version = config
        .worker_version
        .unwrap_or_else(|| crate_version!().to_owned());

    // Initialize app logger as well as Sentry integration
    // Return value *must* be kept in a variable or else it will be dropped and Sentry integration won't work
    let _sentry = sentry_integration::init(crate_name!(), &config.sentry_dsn);

    info!(
        "Starting {} {} [{}]",
        crate_name!(),
        &worker_version,
        &worker_id
    );

    debug!("Connecting to database...");
    let mut conn = PgConnection::connect_with(
        &PgConnectOptions::from_str(&config.database_url)?.application_name(&format!(
            "{}-{}-{}",
            crate_name!(),
            &worker_version,
            &worker_id
        )),
    )
    .await?;
    info!("Connected to database");

    info!("Begin looking for work");
    loop {
        trace!("Fetching next unprocessed request attempt...");
        let mut tx = conn.begin().await?;
        let next_attempt = sqlx::query_as!(RequestAttempt, "
            SELECT ra.request_attempt__id, ra.event__id, ra.subscription__id, ra.created_at, ra.retry_count, t_http.method AS http_method, t_http.url AS http_url, t_http.headers AS http_headers, e.payload AS payload, e.payload_content_type__name AS payload_content_type
            FROM webhook.request_attempt AS ra
            INNER JOIN webhook.subscription AS s ON s.subscription__id = ra.subscription__id
            INNER JOIN webhook.target_http AS t_http ON t_http.target__id = s.target__id
            INNER JOIN event.event AS e ON e.event__id = ra.event__id
            WHERE succeeded_at IS NULL AND failed_at IS NULL AND (delay_until IS NULL OR delay_until <= statement_timestamp())
            ORDER BY created_at ASC
            LIMIT 1
            FOR UPDATE OF ra
            SKIP LOCKED
        ")
        .fetch_optional(&mut tx)
        .await?;

        if let Some(attempt) = next_attempt {
            // Set picked_at
            debug!("Picking request attempt {}", &attempt.request_attempt__id);
            sqlx::query!(
                "
                UPDATE webhook.request_attempt
                SET picked_at = statement_timestamp(), worker_id = $1, worker_version = $2
                WHERE request_attempt__id = $3
                ",
                &worker_id,
                &worker_version,
                attempt.request_attempt__id
            )
            .execute(&mut tx)
            .await?;
            info!("Picked request attempt {}", &attempt.request_attempt__id);

            // Work
            let response = work(&attempt).await;
            debug!(
                "Got a response for request attempt {} in {} ms",
                &attempt.request_attempt__id,
                &response.elapsed_time_ms()
            );

            // Store response
            debug!(
                "Storing response for request attempt {}",
                &attempt.request_attempt__id
            );
            let response_id = sqlx::query!("
                INSERT INTO webhook.response (response_error__name, http_code, headers, body, elapsed_time_ms)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING response__id
            ",
                response.response_error__name(),
                response.http_code(),
                response.headers(),
                response.body,
                response.elapsed_time_ms(),
            )
            .fetch_one(&mut tx)
            .await?
            .response__id;

            // Associate response and request attempt
            debug!(
                "Associating response {} with request attempt {}",
                &response_id, &attempt.request_attempt__id
            );
            #[allow(clippy::suspicious_else_formatting)] // Clippy false positive
            sqlx::query!(
                "UPDATE webhook.request_attempt SET response__id = $1 WHERE request_attempt__id = $2",
                response_id, attempt.request_attempt__id
            )
            .execute(&mut tx)
            .await?;

            if response.is_success() {
                // Mark attempt as completed
                debug!(
                    "Completing request attempt {}",
                    &attempt.request_attempt__id
                );
                sqlx::query!(
                    "UPDATE webhook.request_attempt SET succeeded_at = statement_timestamp() WHERE request_attempt__id = $1",
                    attempt.request_attempt__id
                )
                .execute(&mut tx)
                .await?;

                info!(
                    "Request attempt {} was completed sucessfully",
                    &attempt.request_attempt__id
                );
            } else {
                // Mark attempt as failed
                debug!("Failing request attempt {}", &attempt.request_attempt__id);
                sqlx::query!(
                    "UPDATE webhook.request_attempt SET failed_at = statement_timestamp() WHERE request_attempt__id = $1",
                    attempt.request_attempt__id
                )
                .execute(&mut tx)
                .await?;

                // Creating a retry request
                let retry_count = u32::try_from(attempt.retry_count).unwrap_or(1);
                let retry_in: Duration =
                    min(MINIMUM_RETRY_DELAY * retry_count, MAXIMUM_RETRY_DELAY);
                let next_retry_count = attempt.retry_count + 1;
                let retry_id = sqlx::query!(
                    "
                    INSERT INTO webhook.request_attempt (event__id, subscription__id, delay_until, retry_count)
                    VALUES ($1, $2, statement_timestamp() + $3, $4)
                    RETURNING request_attempt__id
                ",
                    attempt.event__id,
                    attempt.subscription__id,
                    PgInterval::try_from(retry_in).unwrap(),
                    next_retry_count,
                )
                .fetch_one(&mut tx)
                .await?
                .request_attempt__id;

                info!(
                    "Request attempt {} failed; retry #{} created as {} to be picked in {}s",
                    &attempt.request_attempt__id,
                    &next_retry_count,
                    &retry_id,
                    &retry_in.as_secs()
                );
            }
        } else {
            trace!("No unprocessed attempt found");
            delay_for(POLLING_SLEEP).await;
        }

        // Commit transaction
        tx.commit().await?;
    }
}
