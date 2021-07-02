export function mapInit(callback) {
    window.map = L.map('map').setView([51.67204, 39.1843], 13);
    L.tileLayer('https://api.mapbox.com/styles/v1/{id}/tiles/{z}/{x}/{y}?access_token={accessToken}', {
        attribution: 'Map data &copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors, Imagery © <a href="https://www.mapbox.com/">Mapbox</a>',
        maxZoom: 18,
        id: 'mapbox/streets-v11',
        tileSize: 512,
        zoomOffset: -1,
        accessToken: 'pk.eyJ1IjoiZXRhdGFya2luIiwiYSI6ImNrNXh3aTN2ZTA1OXgza3AzM3J3dW52bDgifQ.LyoBwR8ixePf-5erIXhKRg'
    }).addTo(window.map);
    window.map.doubleClickZoom.disable();
    
    // event handler
    window.map.on('dblclick', function(event) {
        if (window.marker) {
            window.map.removeLayer(window.marker);
        }
        window.marker = new L.CircleMarker(event.latlng, 10).addTo(window.map);
        callback(event.latlng.lat, event.latlng.lng);
    });
};

export function mapMove(lat, lng) {
    if (window.map) {
        window.map.setView([lat, lng], 11, { animation: true });
    }
};
