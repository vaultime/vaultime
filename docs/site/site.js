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
