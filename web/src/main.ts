import WebRtcManager from './connection';
import { startCapture } from './input';
import './style.css'

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <div>
    <a href="https://vitejs.dev" target="_blank">
      <img src="/vite.svg" class="logo" alt="Vite logo" />
    </a>
    <h1>Vite + TypeScript</h1>
    <div class="card">
      <button id="share_screen" type="button">Démarrer la capture</button>
      <video id="video" autoplay></video>
    </div>
    <p class="read-the-docs">
      Click on the Vite and TypeScript logos to learn more
    </p>
  </div>
`;

// @ts-ignore
window.manager = new WebRtcManager();
// @ts-ignore
window.manager.connect();
// @ts-ignore
document.querySelector<HTMLButtonElement>("#share_screen")!.addEventListener('click', () => window.manager.start(1))
