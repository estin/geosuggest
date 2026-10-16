const initializeMap = () => {
  const element = document.getElementById("map");
  if (!element || !window.L) return;

  const map = L.map(element, { doubleClickZoom: false }).setView([51.67204, 39.1843], 12);
  map.attributionControl.setPrefix(false);
  L.tileLayer("https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}", {
    attribution: "Tiles &copy; Esri",
    maxZoom: 19,
  }).addTo(map);

  let marker;
  const setMarker = (lat, lng) => {
    if (marker) map.removeLayer(marker);
    marker = L.circleMarker([lat, lng], { radius: 8, weight: 2 }).addTo(map);
  };

  const setCoordinate = (id, value) => {
    const input = document.getElementById(id);
    if (!input) return;
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  };

  map.on("dblclick", (event) => {
    const lat = event.latlng.lat;
    const lng = event.latlng.lng;
    setMarker(lat, lng);
    setCoordinate("latitude", lat.toFixed(5));
    setCoordinate("longitude", lng.toFixed(5));
  });

  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element)) return;
    const city = event.target.closest("[data-map-city]");
    if (city) {
      const lat = Number(city.dataset.lat);
      const lng = Number(city.dataset.lng);
      if (Number.isFinite(lat) && Number.isFinite(lng)) {
        setMarker(lat, lng);
        map.setView([lat, lng], 12, { animate: true });
      }
      return;
    }

    if (event.target.closest("[data-find-city]")) {
      const lat = Number(document.getElementById("latitude")?.value);
      const lng = Number(document.getElementById("longitude")?.value);
      if (Number.isFinite(lat) && Number.isFinite(lng)) {
        setMarker(lat, lng);
        map.setView([lat, lng], 12, { animate: true });
      }
    }
  });

  window.setTimeout(() => map.invalidateSize(), 0);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initializeMap, { once: true });
} else {
  initializeMap();
}
