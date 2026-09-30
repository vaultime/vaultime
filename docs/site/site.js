// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Shows the released version next to the downloads. Before the first release
// there is no latest.json, so it says so and hides the buttons, which would
// lead nowhere. Any other failure only leaves out the version line, because
// the buttons point at the latest downloads and may still work.
(async () => {
  const status = document.getElementById("release");
  if (!status) return;
  try {
    const response = await fetch("/downloads/latest.json", { cache: "no-cache" });
    if (response.status === 404) {
      status.textContent = "The first release is still being tested. Apply for the cloud beta below to hear from us when it is out.";
      document.querySelectorAll("#download .buttons").forEach((row) => {
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
