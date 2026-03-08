import { Router, Request, Response } from 'express';
import { StreamManager } from '../stream-manager';

let streamCounter = 0;

export function createAudioStreamRouter(streamManager: StreamManager): Router {
  const router = Router();

  router.get('/stream', (req: Request, res: Response) => {
    const clientId = `http-client-${++streamCounter}`;
    console.log(`[audio-stream] New client connected: ${clientId}`);

    res.writeHead(200, {
      'Content-Type': 'audio/wav',
      'Transfer-Encoding': 'chunked',
      'Cache-Control': 'no-cache, no-store',
      Connection: 'keep-alive',
    });

    const wavStream = streamManager.createWavStream(clientId);

    wavStream.pipe(res);

    req.on('close', () => {
      console.log(`[audio-stream] Client disconnected: ${clientId}`);
      streamManager.unsubscribe(clientId);
      wavStream.destroy();
    });

    req.on('error', () => {
      streamManager.unsubscribe(clientId);
      wavStream.destroy();
    });
  });

  return router;
}
