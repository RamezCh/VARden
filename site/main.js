// Picks the right download for this computer from the latest GitHub release,
// and plays the terminal demo once when it comes into view.
(function () {
  document.documentElement.classList.add("js");

  // ---------- terminal demo ----------
  const term = document.getElementById("term");
  if (term) {
    const play = () => term.classList.add("play");
    if ("IntersectionObserver" in window) {
      const io = new IntersectionObserver((entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          play();
          io.disconnect();
        }
      }, { threshold: 0.5 });
      io.observe(term);
    } else {
      play();
    }
  }

  // ---------- downloads ----------
  const REPO = "RamezCh/VARden";
  const ua = navigator.userAgent;
  const os = /Windows/i.test(ua) ? "windows" : /Mac OS X|Macintosh/i.test(ua) ? "macos" : /Linux|X11/i.test(ua) && !/Android/i.test(ua) ? "linux" : null;
  const OS_LABEL = { windows: "Windows", macos: "macOS", linux: "Linux" };

  // Which asset each button points to.
  const PICK = {
    windows: (n) => /_x64-setup\.exe$/.test(n),
    macos: (n) => /\.dmg$/.test(n),
    linux: (n) => /\.AppImage$/.test(n),
    deb: (n) => /\.deb$/.test(n),
    rpm: (n) => /\.rpm$/.test(n),
  };

  if (os) {
    const card = document.querySelector(`.platforms [data-os="${os}"]`);
    if (card) card.classList.add("mine");
  }

  const mb = (bytes) => `${(bytes / 1048576).toFixed(0)} MB`;

  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } })
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((release) => {
      const version = String(release.tag_name || "").replace(/^v/, "");
      const assets = release.assets || [];
      for (const link of document.querySelectorAll("[data-asset]")) {
        const asset = assets.find((a) => PICK[link.dataset.asset](a.name));
        if (asset) link.href = asset.browser_download_url;
      }
      const released = release.published_at ? new Date(release.published_at).toLocaleDateString(undefined, { dateStyle: "long" }) : "";
      const line = document.getElementById("release-line");
      if (line && version) line.textContent = `Version ${version}${released ? `, released ${released}` : ""}. Free for Windows, macOS and Linux.`;

      // The hero button downloads straight away for this computer.
      const main = document.getElementById("download-main");
      const note = document.getElementById("download-note");
      const mine = os && assets.find((a) => PICK[os](a.name));
      if (main && mine) {
        main.href = mine.browser_download_url;
        main.textContent = `Download for ${OS_LABEL[os]}`;
        if (note) {
          note.innerHTML = "";
          note.append(`Version ${version}, ${mb(mine.size)}. `);
          const other = document.createElement("a");
          other.href = "#download";
          other.textContent = "Other systems";
          note.append(other);
        }
      }
    })
    .catch(() => {
      // No published release yet (or offline): point to building from source.
      const notice = document.getElementById("no-release");
      if (notice) notice.hidden = false;
      for (const link of document.querySelectorAll(".platforms .btn")) {
        link.classList.add("disabled");
        link.setAttribute("aria-disabled", "true");
        link.textContent = "Coming with the first release";
      }
    });
})();
