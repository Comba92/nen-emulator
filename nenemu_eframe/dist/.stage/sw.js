var cacheName = 'egui-template-pwa';
var filesToCache = [
  './',
  './index.html',
  './nenemu_eframe.js',
  './nenemu_eframe_bg.wasm',
];

/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function (e) {
  e.waitUntil(
    caches.open(cacheName).then(function (cache) {
      return cache.addAll(filesToCache);
    })
