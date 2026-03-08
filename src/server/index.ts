import express from 'express';
import { StreamManager } from '../stream-manager';
import { createApiRouter } from './api';
import { createAudioStreamRouter } from './audio-stream';
import { createUiRouter } from './ui';

export function createServer(
  streamManager: StreamManager,
  getMultiplexer: () => any,
  port: number
): express.Express {
  const app = express();

  app.use(express.json());

  // REST API
  app.use('/api', createApiRouter(getMultiplexer));

  // Audio stream endpoint
  app.use('/audio', createAudioStreamRouter(streamManager));

  // Web dashboard (last, so it doesn't intercept /api or /audio routes)
  app.use('/', createUiRouter());

  return app;
}
