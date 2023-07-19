import * as html from './html';

export function registerOnLoad(fn) {
    let loaded = false;

    function handler() {
        if (loaded) {
            return;
        }
        let readyState = document.readyState;
        if (readyState === 'complete' || readyState === 'interactive') {
            loaded = true;
            fn(); // already loaded
        }
    }

    // document.addEventListener('DOMContentLoaded', fn);
    document.addEventListener('readystatechange', handler);

    handler(); // check now in case already loaded
}

/**
 * Allow throwing errors as expressions
 */
export function throwError(message) {
    throw new Error(message);
}

export function fatalError(err) {
    console.error(err);

    function getMessage() {
        if (typeof err === 'string') {
            return `Error: ${err}`;
        } else if (typeof err.message === 'string') {
            return `Error: ${err.message}`;
        } else {
            return 'Unknown error';
        }
    }

    document.body.innerText = '';
    document.body.appendChild(html.h1(getMessage()));
}

export class UnhandledCaseError extends Error {
    constructor(value) {
        super(`Unhandled case: ${value}`);
    }
}