const colorPalette = [
    "#7b8d43",
    "#ada63e",
    "#a27943",
    "#8a5d3c",
    "#eabe5b",
    "#edefe2",
    "#ea92a8",
    "#5587bc",
    "#4ec0c9",
    "#8d4986",
    "#4e5a98"
];


let logDates = [];
selectedDate = "today";


function logFragment(time, nick, message) {
    return `<div class="message">
    <a id="103" class="time" href="#103">[${time}]</a>
    <span class="from">&lt;${nick}&gt;</span>
    <span class="text">${message}</span>
    </div>`;
}

function logDateFragment(filename) {
    return `<a><img style="margin-right: 3px;" class="icon"
    src="images/file.png">${filename}</a>`;
}

const contents = document.querySelector(".contents");

function colorize() {
    const nicks = document.querySelectorAll(".from");
    nicks.forEach(element => {
        const hashCode = hashCodeFromString(element.textContent);
        const color = getColorIndex(hashCode);
        element.style.color = colorPalette[color];
    });
}

function getColorIndex(hashCode) {
    return Math.abs(hashCode) % colorPalette.length;
}

function hashCodeFromString(str) {
    let hash = 0;
    if (str.length === 0) return hash;
    for (let i = 0; i < str.length; i++) {
        const char = str.charCodeAt(i);
        hash = (hash << 5) - hash + char;
    }
    return hash;
}

async function getLogsDates() {
    const response = await fetch("/dates");
    const dates = await response.json();

    logDates = dates;
}

function createLogDateElement(date) {
    const dateElement = document.createElement("a");
    dateElement.href = "#/" + date;
    dateElement.classList.add("log-date");
    dateElement.innerHTML = logDateFragment(date);
    return dateElement;
}

function loading() {
    const header = document.querySelector(".head");
    header.style.display = "none";

    contents.innerHTML = `<div class="loading"><h1>LOADING</h1></div>`;
}

function finishLoading() {
    contents.innerHTML = "";

    for (const date in groupedLogs) {
        contents.innerHTML += `<p>from ${date}</p>`;
        groupedLogs[date].forEach(element => {
            contents.innerHTML += logFragment(element.time.split("T")[1], element.author, element.body);
        });
    }
}

async function getLogsSearch(searchQuery = "") {
    endpoint = "/search?q=" + searchQuery;

    const response = await fetch(endpoint);
    const logs = await response.json();

    const groupedLogs = {};
    logs.forEach(element => {
        const date = element.time.split("T")[0];
        if (!groupedLogs[date]) {
            groupedLogs[date] = [];
        }
        groupedLogs[date].push(element);
    });
}

async function getLogs(date = "latest") {
    let endpoint = "logs/" + date;

    const response = await fetch(endpoint);
    const logs = await response.json();
    contents.innerHTML = "";

    logs.forEach(element => {
        contents.innerHTML += logFragment(element.time.split("T")[1], element.author, element.body);
    });
    colorize();

    const objDiv = document.querySelector(".contents");
    objDiv.scrollTop = objDiv.scrollHeight;

    header.style.display = "flex";
}


addEventListener("DOMContentLoaded", async () => {
    const dateInput = document.querySelector("#input-date");
    const searchInput = document.querySelector("#input-search");
    const searchButton = document.querySelector("#search");

    dateInput.addEventListener("input", async (event) => {
        await getLogs(event.target.value);
    });
    

    searchButton.addEventListener("click", async (event) => {
        await getLogs("", searchInput.value);
    });

    await getLogs();
    await getLogsDates();

    dateInput.setAttribute("min", logDates[logDates.length - 1]);
    dateInput.setAttribute("max", logDates[0]);
});