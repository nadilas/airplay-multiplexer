import axios from 'axios';

// Sonos API URL (ensure that node-sonos-http-api is running)
const sonosUrl = 'http://localhost:5005/';

// This function will send a play command to Sonos
export async function playSonosTrack(trackUri) {
  try {
    await axios.post(`${sonosUrl}play`, { uri: trackUri });
    console.log('Playing track on Sonos');
  } catch (error) {
    console.log('Error playing track on Sonos:', error);
  }
}
