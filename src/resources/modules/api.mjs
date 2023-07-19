/**
 * @returns {Promise<any>}
 */
export function getTasks() {
    return fetch('/api/tasks', {
        method: 'GET',
        headers: {
            'Accept': 'application/json'
        }
    }).then(handleJsonResponse);
}

/**
 * @returns {Promise<any>}
 */
export function getTask(name) {
    return fetch(`/api/tasks/${name}`, {
        method: 'GET',
        headers: {
            'Accept': 'application/json'
        }
    }).then(handleJsonResponse);
}

/**
 * @param response {Response}
 * @returns {Promise<any>}
 */
function handleJsonResponse(response) {
    if (response.ok) {
        let contentType = response.headers.get('Content-Type');
        if (!contentType || !response.body) {
            return Promise.reject(new Error('Response missing content type or body'));
        } else if (contentType.startsWith('application/json')) {
            return response.json();
        } else {
            return Promise.reject(new Error(`Unexpected response type: ${contentType}`));
        }
    } else {
        return Promise.reject(new ServerError(`Server error: ${response.statusText}`, response.status));
    }
}

class ServerError extends Error {
    constructor(message, status) {
        super(message);
        this.status = status;
    }
}