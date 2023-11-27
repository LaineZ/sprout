const colorPalette = [
    'var(--red)',
    'var(--green)',
    'var(--yellow)',
    'var(--blue)',
    'var(--purple)',
    'var(--aqua)',
    'var(--gray)',
    'var(--orange)',
    'var(--red-dim)',
    'var(--green-dim)',
    'var(--yellow-dim)',
    'var(--blue-dim)',
    'var(--purple-dim)',
    'var(--aqua-dim)',
    'var(--gray-dim)',
    'var(--orange-dim)'
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
    const errorModal = document.createElement("div");
    errorModal.className = "error-modal";
    errorModal.innerHTML = `<h1>${message}</h1>`;

    document.body.appendChild(errorModal);

    setTimeout(function () {
        errorModal.classList.add("closing");
    }, clamp(message.length * 50, 1000));

    setTimeout(function () {
        errorModal.remove();
    }, clamp(message.length * 60, 2000));

    var a = new Audio("sounds/error.wav");
    a.play();
}

function renderIRCFormatting(text) {
    text = text.replace(/\x03(\d{1,2}(,\d{1,2})?)?/g, function (match, color) {
        if (color) {
            return `<span style="color: ${getHTMLColorFromIRC(color)};">`;
        } else {
            return '</span>';
        }
    });

    text = text.replace(/\x0F/g, '</span>');
    text = text.replace(/\x02(.*?)\x02/g, '<strong>$1</strong>');
    text = text.replace(/\x1D(.*?)\x1D/g, '<em>$1</em>');

    return text;
}

function autolinkText(text) {
    const urlRegex = /(\b(https?|ftp|file):\/\/[-A-Z0-9+&@#/%=~_|$?!:,.]*[A-Z0-9+&@#/%=~_|$])/gi;
    return text.replace(urlRegex, '<a href="$1" target="_blank" rel="noopener noreferrer">$1</a>');
}

function getHTMLColorFromIRC(ircColor) {
    const ircColors = [
        'var(--fg)', 'var(--bg1)', 'var(--blue-dim)', 'var(--green-dim)', 'var(--red-dim)', 'var(--brown-dim)', 'var(--mageneta-dim)', 'var(--orange-dim)', 'var(--yellow-dim)',
        'var(--green)', 'var(--aqua-dim)', 'var(--aqua)', 'var(--blue)', 'var(--purple)', 'var(--gray)', 'var(--gray-dim)'
    ];

    const colorCodes = ircColor.split(',').map(Number);
    let htmlColor = "var(--fg)";

    if (colorCodes.length === 1) {
        htmlColor = `${ircColors[colorCodes[0]]}`;
    } else if (colorCodes.length === 2) {
        htmlColor = `${ircColors[colorCodes[1]]}`;
    }

    return htmlColor;
}

function colorize() {
    const nicks = document.querySelectorAll(".from");
    nicks.forEach(element => {
        const hashCode = hashCodeFromString(element.textContent);
        const color = getColorIndex(hashCode);
        element.style.color = colorPalette[color];
    });

    const messages = document.querySelectorAll(".text");
    messages.forEach(element => {
        element.innerHTML = autolinkText(element.textContent);
        element.innerHTML = renderIRCFormatting(element.innerHTML);
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

    document.querySelector("#collapse").classList.remove("hidden");
}

function searchView(path) {
    const searchInput = document.querySelector("#input-search");
    const query = new URLSearchParams(window.location.search);
    searchInput.value = query.get("q");
}

function resize() {
    const collapseButton = document.querySelector("#collapse");

    document.querySelectorAll(".log-date-control").forEach((element) => {
        element.classList.remove("hidden");
    });

    if (window.innerWidth <= 800) {
        document.querySelector("#search").classList.add("hidden");
        collapseButton.classList.remove("collapsed");
    } else {
        document.querySelector("#search").classList.remove("hidden");
    }
}

async function defaultView(path) {
    const logNextButton = document.querySelector("#log-next");
    const logPreviousButton = document.querySelector("#log-prev");
    const dateInput = document.querySelector("#input-date");
    const collapseButton = document.querySelector("#collapse");

    logNextButton.setAttribute("disabled", true);
    logPreviousButton.setAttribute("disabled", true);
    dateInput.setAttribute("disabled", true);

    logNextButton.addEventListener("click", async (event) => {
        if (uiState.currentDateIndex <= 0) {
            uiState.currentDateIndex = 0;
            showErrorModal("Already at the begining");
        } else {
            uiState.currentDateIndex -= 1;
            window.location.href = logDates[uiState.currentDateIndex];
        }
    });

    collapseButton.addEventListener("click", () => {
        document.querySelectorAll(".log-date-control").forEach((element) => {
            collapseButton.classList.toggle("collapsed");

            if (collapseButton.classList.contains("collapsed")) {
                document.querySelector("#search").classList.remove("hidden");
                element.classList.add("hidden");
            } else {
                document.querySelector("#search").classList.add("hidden");
                element.classList.remove("hidden");
            }
        });
    });

    window.addEventListener("resize", () => {
        resize();
    });

    resize();

    logPreviousButton.addEventListener("click", (event) => {
        if (uiState.currentDateIndex > logDates.length - 1) {
            uiState.currentDate = logDates.length - 1;
            showErrorModal("Already at the end");
        } else {
            uiState.currentDateIndex += 1;
            window.location.href = logDates[uiState.currentDateIndex];
        }
    });

    dateInput.addEventListener("change", (event) => {
        window.location.href = event.target.value;
    });

    await getLogsDates();
    uiState.currentDateIndex = clamp(logDates.indexOf(path), 0, logDates.length - 1);

    dateInput.setAttribute("min", logDates[logDates.length - 1]);
    dateInput.setAttribute("max", logDates[0]);

    logNextButton.removeAttribute("disabled");
    logPreviousButton.removeAttribute("disabled");
    dateInput.removeAttribute("disabled");
}

addEventListener("DOMContentLoaded", async () => {
    colorize();
    const path = window.location.pathname.substring(1);

    if (path != "search") {
        if (window.location.hash.length <= 0) {
            const objDiv = document.querySelector(".contents");
            objDiv.scrollTop = objDiv.scrollHeight;
        }
        await defaultView(path.replace("/", ""));
    } else {
        searchView();
    }
});