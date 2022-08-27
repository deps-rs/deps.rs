function buildRepoLink() {
    var formRef = document.forms["repoSelect"];

    var hoster = formRef.elements["hosterSelect"].value.toLowerCase();
    var owner = formRef.elements["owner"].value;
    var repoName = formRef.elements["repoName"].value;

    if (hoster === "gitea") {
        var baseUrl = formRef.elements["baseUrl"].value;

        // verify that the Base URL is not empty
        if(baseUrl.length == 0) {
            formRef.elements["baseUrl"].classList.add("is-danger");
            document.getElementById("baseUrlHelp").classList.add("is-danger");
            var hostName = formRef.elements["hosterSelect"].value;
            document.getElementById("baseUrlHelp").innerHTML = `A Base URL is required for Hosting Provider ${hostName}.`
            
            return;
        }

        window.location.href = `/repo/${hoster}/${baseUrl}/${owner}/${repoName}`;
    } else {
        window.location.href = `/repo/${hoster}/${owner}/${repoName}`;
    }
}

function buildCrateLink() {
    var formRef = document.forms["crateSelect"];

    var crate = formRef.elements["crateName"].value;
    var crateVer = formRef.elements["crateVersion"].value;

    if (crateVer.length == 0) {
        console.log("Aight, Imma get da crate");
        // default to latest version
        window.location.href = `/crate/${crate}`;
    } else {
        console.log("Got a version??");
        window.location.href = `/crate/${crate}/${crateVer}`;
    }
}
