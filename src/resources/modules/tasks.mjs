import {registerOnLoad, throwError} from "./utils";
import {getTasks} from "./api";
import * as html from "./html";

function renderTask(task) {
    let href = `/web/tasks/${task.name}`;
    return html.li([
        html.h2(
            html.code(
                html.a(task.name, {href}))),
        html.p(task.description)
    ]);
}

async function onLoad() {
    let ulTasks = document.getElementById('tasks') || throwError(`Element not found`);

    let tasksJson = await getTasks();

    if (tasksJson.length) {
        tasksJson.forEach(task => {
            ulTasks.appendChild(renderTask(task));
        });
    } else {
        ulTasks.appendChild(html.li([html.i("No tasks found")]));
    }
}

registerOnLoad(onLoad);
