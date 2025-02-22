import Raumfeld from 'raumfeld';

// Create a Raumfeld instance
const raumfeld = new Raumfeld();

// Play music on Teufel speakers
export async function playTeufelTrack(trackUri) {
  try {
    const devices = await raumfeld.getDevices();
    // Loop through devices and play on the first available one
    await devices[0].play(trackUri);
    console.log('Playing track on Teufel');
  } catch (error) {
    console.log('Error playing track on Teufel:', error);
  }
}
