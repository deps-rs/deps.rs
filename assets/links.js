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
}

function buildCrateLink() {
    let formRef = document.forms["crateSelect"];

    let crate = formRef.elements["crateName"].value;
    let crateVer = formRef.elements["crateVersion"].value;

    if (crateVer.length == 0) {
        // default to latest version
        window.location.assign(`/crate/${crate}`);
    } else {
        window.location.assign(`/crate/${crate}/${crateVer}`);
    }
}
