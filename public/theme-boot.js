(() => {
  const forced = ["light", "dark"];
  let pref = null;
  try {
    const review = new URLSearchParams(window.location.search).get("theme");
    pref = review || window.localStorage.getItem("eepview.theme");
  } catch {
    pref = null;
  }
  if (forced.includes(pref)) document.documentElement.setAttribute("data-theme", pref);
})();
