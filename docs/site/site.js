// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Shows the released version next to the downloads. Without a latest.json the
// downloads of this site are missing, so it says so and hides their buttons,
// which would lead nowhere, while the GitHub ones stay. Any other failure only
// leaves out the version line, because the buttons may still work.
(async () => {
  const status = document.getElementById("release");
  if (!status) return;
  try {
    const response = await fetch("/downloads/latest.json", { cache: "no-cache" });
    if (response.status === 404) {
      status.textContent = "The downloads on this site are not available right now. The releases on GitHub have the same installers.";
      document.querySelectorAll("#download [data-from-site]").forEach((row) => {
        row.hidden = true;
      });
      status.hidden = false;
      return;
    }
    if (!response.ok) return;
    const latest = await response.json();
    const released = latest.pub_date
      ? new Date(latest.pub_date).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric" })
      : null;
    status.textContent = released ? `Version ${latest.version}, out since ${released}.` : `Version ${latest.version}.`;
    status.hidden = false;
  } catch {
    // A network error or a broken latest.json leaves the version line hidden.
  }
})();

// Marks the section of a documentation page that is in view in its list of
// sections.
(() => {
  const links = [...document.querySelectorAll(".docs-toc a")];
  if (links.length === 0) return;
  const headings = links.map((link) => document.getElementById(link.hash.slice(1))).filter(Boolean);
  const mark = () => {
    // A heading counts as reached once it passes the top quarter of the window.
    const line = window.innerHeight / 4;
    let current = headings[0];
    for (const heading of headings) {
      if (heading.getBoundingClientRect().top <= line) current = heading;
    }
    const atEnd = window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - 2;
    if (atEnd) current = headings[headings.length - 1];
    for (const link of links) link.classList.toggle("active", link.hash === `#${current.id}`);
  };
  document.addEventListener("scroll", mark, { passive: true });
  mark();
})();
