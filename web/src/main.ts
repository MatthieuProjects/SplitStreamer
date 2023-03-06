import WebRtcManager, { Status } from './connection';
import './index.css';

const manager = new WebRtcManager();

const video = document.querySelector<HTMLVideoElement>('#video')!;
const status = document.querySelector<HTMLSpanElement>('#status')!;
const start = document.querySelector<HTMLButtonElement>('#start')!;

// Add an event handler when we want to start streaming.
start.addEventListener('click', () => {
  console.log(':: User clicked the button');

  manager.transmit();
});

// When the manager changes his status.
manager.on('status', (newStatus: Status) => {
  const shouldDisplayButton = newStatus === Status.Ready;
  start.disabled = !shouldDisplayButton;

  if (newStatus === Status.Transmitting) {
    // We need to display the video stream!
  }

  status.textContent = newStatus;
});

manager.on('video', (stream: MediaStream) => {
  console.log('--> Video initialized', stream);
  video.srcObject = stream;
});
