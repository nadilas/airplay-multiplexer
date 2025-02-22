import { Device } from 'airplay-protocol';

// Assuming you have the HomePod device information
export const homePod = new Device('HomePod-IP');  // Use the actual IP of your HomePod

// HomePod's IP address
const homePodIp = 'HomePod-IP';  // Replace with actual IP address of the HomePod

// Discover and connect to HomePod
const device = new Device(homePodIp);  // This creates a new device instance for the HomePod

device.on('deviceReady', () => {
  console.log('Connected to HomePod');
  // Now we can play or control the HomePod
  device.play('http://path-to-audio-or-stream', 0, (err) => {
    if (err) {
      console.log('Error playing stream on HomePod:', err);
    } else {
      console.log('Streaming to HomePod');
    }
  });
});

device.on('error', (err) => {
  console.error('Error connecting to HomePod:', err);
});

// Initiate connection to HomePod
device.connect();