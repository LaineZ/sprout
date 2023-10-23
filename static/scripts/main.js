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

    document.querySelectorAll(".log-date-control").forEach((element) => {
        element.classList.remove("hidden");
    });
}

function searchView(path) {
    const searchInput = document.querySelector("#input-search");
    const query = new URLSearchParams(window.location.search);
    searchInput.value = query.get("q");
}

async function defaultView(path) {
    const logNextButton = document.querySelector("#log-next");
    const logPreviousButton = document.querySelector("#log-prev");
    const dateInput = document.querySelector("#input-date");

    logNextButton.addEventListener("click", async (event) => {
        if (uiState.currentDateIndex <= 0) {
            uiState.currentDateIndex = 0;
            showErrorModal("Already at the beggining");
        } else {
            uiState.currentDateIndex -= 1;
            window.location.href = logDates[uiState.currentDateIndex];
        }
    });

    logPreviousButton.addEventListener("click", async (event) => {
        if (uiState.currentDateIndex > logDates.length - 1) {
            uiState.currentDate = logDates.length - 1;
            showErrorModal("Already at the end");
        } else {
            uiState.currentDateIndex += 1;
            window.location.href = logDates[uiState.currentDateIndex];
        }
    });
    
    dateInput.addEventListener("input", async (event) => {
        window.location.href = event.target.value;
    });
    
    await getLogsDates();
    uiState.currentDateIndex = clamp(logDates.indexOf(path), 0, logDates.length - 1);

    dateInput.setAttribute("min", logDates[logDates.length - 1]);
    dateInput.setAttribute("max", logDates[0]);
}

addEventListener("DOMContentLoaded", async () => {
    colorize();
    const path = window.location.pathname.substring(1);

    if (path != "search") {
        if (window.location.hash.length > 0) {
            const objDiv = document.querySelector(".contents");
            objDiv.scrollTop = objDiv.scrollHeight;
        }
        await defaultView(path);
    } else {
        searchView();
    }
});