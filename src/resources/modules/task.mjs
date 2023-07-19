import {registerOnLoad, throwError, UnhandledCaseError} from "./utils";
import {getTask} from "./api";
import * as html from "./html";
import {fatalError} from "./utils";

/**
 * @returns {HTMLElement}
 */
function renderInput(parameter) {
    let inputId = `parameter_${parameter.name}`;

    let attributes = {
        id: inputId,
        name: parameter.name,
        ...(parameter.required === true
            ? {required: true}
            : {}),
        value: parameter.default || ''
    }

    if ((parameter.enum || []).length) {
        return html.select(parameter.enum.map(value => html.option(`${value}`)), {...attributes});
    }

    switch (parameter.type) {
        case 'string':
            return html.input([], {...attributes, type: 'text'});
        case 'number':
            return html.input([], {...attributes, type: 'number'});
        case 'boolean':
            return html.input([], {...attributes, type: 'checkbox'});
        default:
            throw new UnhandledCaseError(parameter.type);
    }
}

const URL_PATTERN = new RegExp('^/web/tasks/([^/]+)$');

function getTaskName(location) {
    let match = URL_PATTERN.exec(location.pathname);
    if (!match) {
        throw new Error(`Unexpected URL path: ${location.pathname}`);
    }
    return match[1];
}

async function onLoad() {
    try {
        let name = getTaskName(window.location);

        let taskJson = await getTask(name);

        let tableParameters = document.getElementById('parameters-table') || throwError(`Element not found`);

        if (taskJson.parameters?.length) {
            taskJson.parameters.forEach(parameter => {
                let inputId = `parameter_${parameter.name}`;
                tableParameters.appendChild(
                    html.tr([
                        html.td(
                            html.label(parameter.name),
                            {'for': inputId}),
                        html.td(renderInput(parameter))
                    ]));
            });
        } else {
            tableParameters.appendChild(
                html.tr(
                    html.td('Task has no parameters', {colspan: 2})));
        }

        let formParameters = document.getElementById('parameters-form') || throwError(`Element not found`);

        if (taskJson.method.includes('POST')) {
            formParameters.method = 'POST';
        } else if (taskJson.method.includes('GET')) {
            formParameters.method = 'GET';
        } else {
            throw new Error(`Missing form method`);
        }

        formParameters.action = `/api/tasks/${name}/run`;

        let codeTaskName = document.getElementById('task-name') || throwError(`Element not found`);

        codeTaskName.innerText = name;

    } catch (err) {
        fatalError(err);
    }
}

registerOnLoad(onLoad);
