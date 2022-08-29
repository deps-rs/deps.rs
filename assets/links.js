function buildRepoLink() {
    let formRef = document.forms["repoSelect"];

    let hoster = formRef.elements["hosterSelect"].value.toLowerCase();
    let owner = formRef.elements["owner"].value;
    let repoName = formRef.elements["repoName"].value;

    if (hoster === "gitea") {
        let baseUrl = formRef.elements["baseUrl"].value;

        // verify that the Base URL is not empty
        if(baseUrl.length == 0) {
            formRef.elements["baseUrl"].classList.add("is-danger");
            document.getElementById("baseUrlHelp").classList.add("is-danger");
            let hostName = formRef.elements["hosterSelect"].value;
            document.getElementById("baseUrlHelp").textContent = `A Base URL is required for Hosting Provider ${hostName}.`
            
            return;
        }

        window.location.href = `/repo/${hoster}/${baseUrl}/${owner}/${repoName}`;
    } else {
        window.location.href = `/repo/${hoster}/${owner}/${repoName}`;
    }
}

function buildCrateLink() {
    let formRef = document.forms["crateSelect"];

    let crate = formRef.elements["crateName"].value;
    let crateVer = formRef.elements["crateVersion"].value;

    if (crateVer.length == 0) {
        console.log("Aight, Imma get da crate");
        // default to latest version
        window.location.href = `/crate/${crate}`;
    } else {
        console.log("Got a version??");
        window.location.href = `/crate/${crate}/${crateVer}`;
    }
}
