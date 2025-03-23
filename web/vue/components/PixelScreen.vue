<script setup>
import UsersSidebar from '@/components/user/UsersSidebar.vue';
import { parse } from 'protobufjs';
import { ZstdCodec } from 'zstd-codec';
import { ref } from 'vue';

const currentUser = ref('');
const users = ref([]);

let currentScreenWidth;
let currentScreenHeight;

const protoSchema = `
syntax = "proto3";

package pixelstrom;

// Root message for WebSocket communication
message WebSocketMessage {
  oneof payload {
    WebSocketClosedBecauseOfLag web_socket_closed_because_of_lag = 1;
    ScreenSync screen_sync = 2;
    UserPainting user_painting = 3;
    CurrentlyPaintingUser currently_painting_user = 4;
    UserStatisticsUpdate user_statistics_update = 5;
  }
}

// The websocket connection lagged behind to much, so it was closed.
// This can happen because of a too slow network connection or imperforate browser.
message WebSocketClosedBecauseOfLag {
    uint64 lag = 1;
}

// Entire contents of the screen
message ScreenSync {
    uint32 width = 1;
    uint32 height = 2;
    // width * height * 4 bytes (rgba)
    bytes pixels = 3;
}

// Partial update of the screen after a user finished painting
message UserPainting {
    // Name of the user that painted the pixels
    string username = 1;

    // List of (2 byte x + 2 byte y + 4 byte (rgba)).
    // Contains multiple entries
    bytes painted = 2;
}

// It's now the turn for a user to paint
message CurrentlyPaintingUser {
    // Name of the currently painting user
    string currentlyPainting = 1;
}

// Get an update on the user statistics.
// This message is also send once a user joins or leaves.
// This makes it easier for the frontend, it can update the users list on every UserStatisticsUpdate
// and the pointer to the currently painting user for every CurrentlyPaintingUser
message UserStatisticsUpdate {
    // List of statistics
    repeated UserStatistics statistics = 1;
}

message UserStatistics {
    // Name of the user the statistics are for
    string username = 1;

    // The number of pixels/s the user is sending
    float pixelsPerSecond = 2;

    // The average response time of the user (Completing drawing is counted as response)
    float averageResponseTimeMilliseconds = 3;
}
`;

// Parse the schema
const root = parse(protoSchema).root;

const ScreenSync = root.lookupType('ScreenSync');
const WebSocketMessage = root.lookupType('WebSocketMessage');

// Wait for the protobuf.js library to load (and other stuff???)
window.onload = () => {
  // Create a WebSocket connection to the server
  const socket = new WebSocket('ws://localhost:3000/ws');

  var received_counter = 0;
  var processed_counter = 0;

  var streamingDecoder;
  ZstdCodec.run((zstd) => {
    streamingDecoder = new zstd.Streaming();
  });
  console.log('Created streaming zstd decompressor');

  socket.onmessage = async (event) => {
    received_counter++;
    const compressed = new Uint8Array(await event.data.arrayBuffer());

    // console.log(
    //   'Got compressed message with',
    //   compressed.length,
    //   'bytes',
    //   'Received:',
    //   received_counter,
    //   'Processed:',
    //   processed_counter,
    //   'Lag:',
    //   received_counter - processed_counter,
    // );

    const decompressed = streamingDecoder.decompress(compressed);
    try {
      const webSocketMessage = WebSocketMessage.decode(decompressed);
      applyWebSocketMessage(webSocketMessage);
    } catch (e) {
      console.error('Error decoding webSocketMessage:', e);
    }

    processed_counter++;
  };

  socket.onerror = (error) => {
    console.error('WebSocket Error:', error);
  };

  socket.onopen = () => {
    console.log('WebSocket connection established');
  };

  socket.onclose = () => {
    console.log('WebSocket connection closed');
  };

  // fetch('http://localhost:3000/api/current-screen')
  fetch(window.location.protocol + '//' + window.location.hostname + ':3000/api/current-screen')
    .then((response) => {
      if (!response.ok) {
        throw new Error('Network response was not ok');
      }
      // Read the response body as an ArrayBuffer
      return response.arrayBuffer();
    })
    .then((arrayBuffer) => {
      // Convert the ArrayBuffer to a Uint8Array to decode with Protobuf
      const bytes = new Uint8Array(arrayBuffer);
      return ScreenSync.decode(bytes);
    })
    .then((screenSync) => {
      // Apply the decoded object
      applyScreenSync(screenSync);
    })
    .catch((error) => {
      console.error('Error fetching initial screen sync:', error);
    });
};

function applyScreenSync(screenSync) {
  currentScreenWidth = screenSync.width;
  currentScreenHeight = screenSync.height;

  if (currentScreenWidth === 0 || currentScreenHeight === 0) {
    console.error('Invalid screenSync dimensions:', currentScreenWidth, currentScreenHeight);
    return;
  }

  // Get pixel data (this assumes pixels are in raw RGBA format)
  const pixels = new Uint8Array(screenSync.pixels);

  // Get the canvas element and its context
  const screen = document.getElementById('screen');
  const ctx = screen.getContext('2d');

  adjustScreenSize();

  // Create ImageData object to hold the pixels
  const imageData = ctx.createImageData(currentScreenWidth, currentScreenHeight);

  for (let byte = 0; byte < pixels.length; byte += 4) {
    imageData.data[byte + 0] = pixels[byte + 0]; // Red
    imageData.data[byte + 1] = pixels[byte + 1]; // Green
    imageData.data[byte + 2] = pixels[byte + 2]; // Blue
    imageData.data[byte + 3] = 255; // Alpha
  }

  // Put the ImageData onto the canvas
  ctx.putImageData(imageData, 0, 0);
}

function applyUserPainting(userPainting) {
  // console.log(userPainting.user, 'painted', userPainting.painted.length / 8, 'pixels');
  const painted = new Uint8Array(userPainting.painted);

  const screen = document.getElementById('screen');
  const ctx = screen.getContext('2d');
  const width = currentScreenWidth;
  const height = currentScreenHeight;

  const imageData = ctx.getImageData(0, 0, width, height);

  // Every message has 8 bytes
  for (let byte = 0; byte < painted.length; byte += 8) {
    const x = (painted[byte + 0] << 8) + painted[byte + 1];
    const y = (painted[byte + 2] << 8) + painted[byte + 3];
    const index = 4 * (y * width + x);

    imageData.data[index + 0] = painted[byte + 5]; // Red
    imageData.data[index + 1] = painted[byte + 6]; // Green
    imageData.data[index + 2] = painted[byte + 7]; // Blue
    imageData.data[index + 3] = 255; // Alpha
  }

  ctx.putImageData(imageData, 0, 0);
}

function applyCurrentlyPaintingUser(currentlyPaintingUser) {
  currentUser.value = currentlyPaintingUser.currentlyPainting;
}

function applyUserStatisticsUpdate(userStatisticsUpdate) {
  users.value = userStatisticsUpdate.statistics;
}

function applyWebSocketMessage(webSocketMessage) {
  // console.log('Got WebSocketMessage', webSocketMessage, 'with payload', webSocketMessage.payload);
  switch (webSocketMessage.payload) {
    case 'webSocketClosedBecauseOfLag':
      alert(
        'Your websocket connection had too much lag, it was closed. Either your network or your browser is too slow and could not handle the pixelstrom :P. Note to myself: Make a nice error box for this',
      );
      break;
    case 'screenSync':
      applyScreenSync(webSocketMessage.screenSync);
      break;
    case 'userPainting':
      applyUserPainting(webSocketMessage.userPainting);
      break;
    case 'currentlyPaintingUser':
      applyCurrentlyPaintingUser(webSocketMessage.currentlyPaintingUser);
      break;
    case 'userStatisticsUpdate':
      applyUserStatisticsUpdate(webSocketMessage.userStatisticsUpdate);
      break;
  }
}

function adjustScreenSize() {
  const screen = document.getElementById('screen');
  const screenContainer = document.getElementById('screen-container');
  const screenContainerWidth = screenContainer.getBoundingClientRect().width;

  // Set the screen size to the screenSync size
  screen.width = currentScreenWidth;
  screen.height = currentScreenHeight;

  screen.style.width = screenContainerWidth + 'px';
  screen.style.height = (screenContainerWidth / currentScreenWidth) * currentScreenHeight + 'px';
}

window.addEventListener('resize', () => {
  adjustScreenSize();
});
</script>

<template>
  <div id="game-container">
    <div id="screen-container">
      <canvas id="screen"></canvas>
    </div>
    <UsersSidebar :current-painting-user="currentUser" :users="users"/>
  </div>
</template>

<style scoped>
#game-container {
  display: flex;
  overflow: hidden;
}

/* https://stackoverflow.com/a/7170097 */
#screen-container {
  /* width and height will be set by JavaScript */
  flex: auto;
  background-color: black;
}

/* Don't use pixel interpolation */
/* https://stackoverflow.com/a/7665647 */
canvas {
  image-rendering: optimizeSpeed; /* Older versions of FF          */
  image-rendering: -moz-crisp-edges; /* FF 6.0+                       */
  image-rendering: -webkit-optimize-contrast; /* Safari                        */
  image-rendering: -o-crisp-edges; /* OS X & Windows Opera (12.02+) */
  image-rendering: pixelated; /* Awesome future-browsers       */
  -ms-interpolation-mode: nearest-neighbor; /* IE                            */
}
</style>
