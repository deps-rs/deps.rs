function buildRepoLink() {
    let formRef = document.forms["repoSelect"];

    let hoster = formRef.elements["hosterSelect"].value.toLowerCase();
    let owner = formRef.elements["owner"].value;
    let repoName = formRef.elements["repoName"].value;
    let innerPath = formRef.elements["innerPath"].value;

    let qparams = "";
    if (innerPath.length > 0) {
        qparams = "?path=" + encodeURIComponent(innerPath);
    }

    if (hoster === "gitea") {
        let baseUrl = formRef.elements["baseUrl"].value;

        // verify that the Base URL is not empty
        if(baseUrl.length === 0) {
            formRef.elements["baseUrl"].classList.add("is-danger");
            document.getElementById("baseUrlHelp").classList.add("is-danger");
            let hostName = formRef.elements["hosterSelect"].value;
            document.getElementById("baseUrlHelp").textContent = `A Base URL is required for Hosting Provider ${hostName}.`
            
            return;
        }

        window.location.assign(`/repo/${hoster}/${baseUrl}/${owner}/${repoName}${qparams}`);
    } else {
        window.location.assign(`/repo/${hoster}/${owner}/${repoName}${qparams}`);
    }

    return false;
}

function buildCrateLink() {
    let formRef = document.forms["crateSelect"];

    let crate = formRef.elements["crateName"].value;
    let crateVer = formRef.elements["crateVersion"].value;

    if (crateVer.length === 0) {
        // default to latest version
        window.location.assign(`/crate/${crate}`);
    } else {
        window.location.assign(`/crate/${crate}/${crateVer}`);
    }

    return false;
}

function activateBadgeTab(root, target) {
    let tabs = root.querySelectorAll("[data-badge-target]");
    let panels = root.querySelectorAll("[data-badge-panel]");

    tabs.forEach(function(tab) {
        let li = tab.closest("li");
        let isActive = tab.dataset.badgeTarget === target;

        if (!li) {
            return;
        }

        li.classList.toggle("is-active", isActive);
        tab.setAttribute("aria-selected", isActive ? "true" : "false");
        tab.tabIndex = isActive ? 0 : -1;
    });

    panels.forEach(function(panel) {
        panel.hidden = panel.dataset.badgePanel !== target;
    });
}

document.addEventListener("DOMContentLoaded", function() {
    document.querySelectorAll("[data-badge-root]").forEach(function(root) {
        let container = root.querySelector("[data-badge-tabs]");
        if (!container) {
            return;
        }

        let tabs = Array.from(container.querySelectorAll("[data-badge-target]"));
        let activeTab =
            tabs.find(function(tab) {
                return tab.getAttribute("aria-selected") === "true";
            }) || tabs[0];
        if (activeTab) {
            activateBadgeTab(root, activeTab.dataset.badgeTarget);
        }

        tabs.forEach(function(tab) {
            tab.addEventListener("click", function() {
                activateBadgeTab(root, tab.dataset.badgeTarget);
            });

            tab.addEventListener("keydown", function(event) {
                let key = event.key;
                if (key !== "ArrowRight" && key !== "ArrowLeft" && key !== "Home" && key !== "End") {
                    return;
                }

                event.preventDefault();
                let currentIndex = tabs.indexOf(tab);
                let nextIndex = currentIndex;
                if (key === "ArrowRight") {
                    nextIndex = (currentIndex + 1) % tabs.length;
                } else if (key === "ArrowLeft") {
                    nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
                } else if (key === "Home") {
                    nextIndex = 0;
                } else if (key === "End") {
                    nextIndex = tabs.length - 1;
                }

                let nextTab = tabs[nextIndex];
                if (!nextTab) {
                    return;
                }

                activateBadgeTab(root, nextTab.dataset.badgeTarget);
                nextTab.focus();
            });
        });
    });
});
