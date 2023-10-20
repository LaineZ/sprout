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

const clamp = (num, min, max = Number.MAX_SAFE_INTEGER) => Math.min(Math.max(num, min), max);

const contents = document.querySelector(".contents");
const header = document.querySelector(".head");


Object.defineProperty(Array.prototype, 'getOrNull', {
    value: function (index) {
        return index > this.length - 1 ? null : this[index];
    }
});

let logDates = [];
let uiState = {
    currentDateIndex: 0,

    set currentDateIndex(value) {
        document.querySelector("#current-date").innerHTML = logDates[value];
        document.querySelector("#input-date").value = logDates[value];
        this._currentDateIndex = value;
    },

    get currentDateIndex() {
        return this._currentDateIndex;
    }
}

function showErrorModal(message) {
    document.querySelector("body").innerHTML += `<div class="error-modal"><h1>${message}</h1></div>`;
    setTimeout(function () {
        document.querySelectorAll(".error-modal").forEach(element => {
            element.classList.add("closing");
        });
    }, clamp(message.length * 50, 1000));

    setTimeout(function () {
        document.querySelectorAll(".error-modal").forEach(element => {
            element.remove();
        });
    }, clamp(message.length * 60, 2000));

    var a = new Audio("sounds/error.wav");
    a.play();
}

function logFragment(time, nick, message, offset, link) {
    return `<div class="message">
    <a id="${offset}" class="time" href="${link}">[${time}]</a>
    <span class="from">&lt;${nick}&gt;</span>
    <span class="text">${message}</span>
    </div>`;
}

function logDateFragment(filename) {
    return `<a><img style="margin-right: 3px;" class="icon"
    src="images/file.png">${filename}</a>`;
}

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

    document.querySelectorAll(".jump-to-date").forEach((element) => {
        element.classList.remove("jump-to-date");
    });
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
    const objDiv = document.querySelector(".contents");
    objDiv.scrollTop = objDiv.scrollHeight;
    header.style.display = "flex";
}

async function getLogsSearch(searchQuery = "") {
    loading();
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

    for (const date in groupedLogs) {
        contents.innerHTML += `<h2>${date}</h2>`;
        groupedLogs[date].forEach(element => {
            contents.innerHTML += logFragment(element.time.split("T")[1], element.author, element.body, element.offset, element.offset);
        });
    }

    colorize();
    finishLoading();
}

async function getLogs(date = "latest") {
    loading();
    let endpoint = "logs/" + (date == null ? "latest" : date);
    const response = await fetch(endpoint);
    const logs = await response.json();
    finishLoading();

    logs.forEach(element => {
        contents.innerHTML += logFragment(element.time.split("T")[1], element.author, element.body, element.offset, element.offset);
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
    const logNextButton = document.querySelector("#log-next");
    const logPreviousButton = document.querySelector("#log-prev");

    logNextButton.addEventListener("click", async (event) => {
        if (uiState.currentDateIndex <= 0) {
            uiState.currentDateIndex = 0;
            showErrorModal("Already at the beggining");
        } else {
            uiState.currentDateIndex -= 1;
            await getLogs(logDates[uiState.currentDateIndex]);
        }
    });

    logPreviousButton.addEventListener("click", async (event) => {
        if (uiState.currentDateIndex > logDates.length - 1) {
            uiState.currentDate = logDates.length - 1;
            showErrorModal("Already at the end");
        } else {
            uiState.currentDateIndex += 1;
            await getLogs(logDates[uiState.currentDateIndex]);
        }
    });

    dateInput.addEventListener("input", async (event) => {
        const idx = logDates.indexOf(event.target.value);
        if (idx != -1) {
            uiState.currentDateIndex = logDates.indexOf(event.target.value);
            console.log(event.target.value);
            await getLogs(event.target.value);
        }
    });


    searchButton.addEventListener("click", async (event) => {
        await getLogsSearch(searchInput.value);
    });

    // #/2023-10-20/124
    //   ^ log path ^ offset
    const path = window.location.hash.split("/");
    const logDate = path.getOrNull(1);
    const logOffset = path.getOrNull(2);

    await getLogs(logDate);
    await getLogsDates();


    const dateId = logDates.indexOf(logDate);
    if (dateId != -1) {
        uiState.currentDateIndex = did;
    } else {
        uiState.currentDateIndex = 0;
    }

    dateInput.setAttribute("min", logDates[logDates.length - 1]);
    dateInput.setAttribute("max", logDates[0]);
});