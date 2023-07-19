export function element(name, children, attributes) {
    let el = document.createElement(name);

    function appendChild(child) {
        if (typeof child === 'string') {
            el.appendChild(text(child));
        } else if (child instanceof HTMLElement) {
            el.appendChild(child);
        } else {
            throw new Error(`Unexpected child: ${typeof child}`);
        }
    }

    if (children) {
        if (Array.isArray(children)) {
            children.forEach(appendChild);
        } else {
            appendChild(children); // assume single child
        }
    }
    if (attributes) {
        for (const [key, value] of Object.entries(attributes)) {
            el.setAttribute(key, value);
        }
    }
    return el;
}

export function text(textContent) {
    return document.createTextNode(textContent);
}

export function h1(children, attributes) {
    return element('h1', children, attributes);
}

export function h2(children, attributes) {
    return element('h2', children, attributes);
}

export function p(children, attributes) {
    return element('p', children, attributes);
}

export function i(children, attributes) {
    return element('i', children, attributes);
}

export function b(children, attributes) {
    return element('b', children, attributes);
}

export function li(children, attributes) {
    return element('li', children, attributes);
}

export function code(children, attributes) {
    return element('code', children, attributes);
}

export function a(children, attributes) {
    return element('a', children, attributes);
}

export function table(children, attributes) {
    return element('table', children, attributes);
}

export function tr(children, attributes) {
    return element('tr', children, attributes);
}

export function td(children, attributes) {
    return element('td', children, attributes);
}

export function label(children, attributes) {
    return element('label', children, attributes);
}

export function input(children, attributes) {
    return element('input', children, attributes);
}

export function select(children, attributes) {
    return element('select', children, attributes);
}

export function option(children, attributes) {
    return element('option', children, attributes);
}
