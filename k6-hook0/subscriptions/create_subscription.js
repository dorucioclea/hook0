import http from 'k6/http';
import { check } from 'k6';

export default function (baseUrl, auth_token, application_id, event_types, target_url) {
    const url = `${baseUrl}/api/v1/subscriptions/`;
    const payload = {
        is_enabled: true,
        metadata: {
            newKey: 'New Value',
        },
        application_id: application_id,
        description: 'Ceci est un test',
        label_key: 'all',
        label_value: 'yes',
        event_types: event_types,
        target: {
            type: 'http',
            method: 'POST',
            url: target_url,
            headers: {}
        }
    };

    const params = {
        headers: {
            'Authorization': auth_token,
            'accept': 'application/json',
            'content-type': 'application/json',
        },
    };

    const res = http.post(url, JSON.stringify(payload), params);
    if(!check(res, {
        'Subscription created': (r) => r.status === 201 && r.body && r.body.includes('created_at') && r.body.includes('subscription_id'),
    })) return null;

    return JSON.parse(res.body).subscription_id;
}