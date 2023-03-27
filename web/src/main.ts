import 'flowbite';
import WebRtcManager, { Status } from './connection';
import './index.css';

const manager = new WebRtcManager();

const video = document.querySelector<HTMLVideoElement>('#video')!;
const status = document.querySelector<HTMLSpanElement>('#status')!;
const start = document.querySelector<HTMLButtonElement>('#start')!;
const stop = document.querySelector<HTMLButtonElement>('#stop')!;

// @ts-ignore
function doStart(type: 'user' | 'display') {
  console.log(':: User clicked the button for ', type);
  manager.transmit(type);
}

// @ts-ignore
function doStop() {
  manager.stopTransmit();
}

const shouldDisplayButton = manager.status === Status.Ready;
start.style.display = !shouldDisplayButton ? 'none' : 'block';
const shouldDisplayStopButton = manager.status === Status.Transmitting;
stop.style.display = !shouldDisplayStopButton ? 'none' : 'block';

// When the manager changes his status.
manager.on('status', (newStatus: Status) => {
  const shouldDisplayButton = manager.status === Status.Ready;
  start.style.display = !shouldDisplayButton ? 'none' : 'block';
  const shouldDisplayStopButton = manager.status === Status.Transmitting;
  stop.style.display = !shouldDisplayStopButton ? 'none' : 'block';

  if (newStatus === Status.Transmitting) {
    // We need to display the video stream!
  }

  status.textContent = newStatus;
});

manager.on('video', (stream: MediaStream) => {
  console.log('--> Video initialized', stream);
  video.srcObject = stream;
});
