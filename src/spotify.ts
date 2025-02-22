import SpotifyWebApi from 'spotify-web-api-node';

// Set up your Spotify credentials here
const spotifyApi = new SpotifyWebApi({
  clientId: '4bef1ef02c0d405fae85c1f154abdf18',
  clientSecret: '7e822fbe30094698aefaed1d8f20101c',
  redirectUri: 'https://ha.apps.janosveres.eu/api/spotify',
});

// This function will authenticate and return the access token
async function authenticateSpotify() {
  try {
    const data = await spotifyApi.clientCredentialsGrant();
    spotifyApi.setAccessToken(data.body['access_token']);
    console.log('Spotify API authenticated');
  } catch (error) {
    console.log('Error during Spotify authentication:', error);
  }
}

// Call the function to authenticate
authenticateSpotify();

export async function playTrackOnSpotify(trackUri) {
    try {
      await spotifyApi.play({ uris: [trackUri] });
      console.log('Playing track:', trackUri);
    } catch (error) {
      console.log('Error playing track on Spotify:', error);
    }
  }
  
