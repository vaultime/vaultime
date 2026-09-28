// Shows the released version next to the downloads, or says there is none
// yet and hides the buttons, which would lead nowhere.
(async () => {
  const status = document.getElementById("release");
  if (!status) return;
  const buttons = document.querySelectorAll("#download .buttons");
  try {
    const response = await fetch("/downloads/latest.json", { cache: "no-cache" });
    if (!response.ok) throw new Error(`status ${response.status}`);
    const latest = await response.json();
    const released = latest.pub_date
      ? new Date(latest.pub_date).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric" })
      : null;
    status.textContent = released ? `Version ${latest.version}, out since ${released}.` : `Version ${latest.version}.`;
  } catch {
    status.textContent = "The first release is still being tested. Apply for the cloud beta below to hear from us when it is out.";
    buttons.forEach((row) => {
      row.hidden = true;
    });
  }
  status.hidden = false;
})();
