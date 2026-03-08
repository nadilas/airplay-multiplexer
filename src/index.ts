import { loadConfig } from './config';
import { AudioMultiplexer } from './multiplexer';
import { createServer } from './server/index';

async function main(): Promise<void> {
  const config = loadConfig();

  console.log('=== Multi-Room Audio Multiplexer ===');
  console.log(`Receiver Name: ${config.receiverName}`);
  console.log(`Local IP: ${config.localIp}`);
  console.log(`HTTP Port: ${config.httpPort}`);
  console.log(`Audio Format: ${config.audioFormat.sampleRate}Hz / ${config.audioFormat.bitDepth}bit / ${config.audioFormat.channels}ch`);
  console.log('====================================\n');

  const multiplexer = new AudioMultiplexer(config);

  // Create HTTP server
  const app = createServer(
    multiplexer.getStreamManager(),
    () => multiplexer,
    config.httpPort
  );

  // Start HTTP server
  const server = app.listen(config.httpPort, '0.0.0.0', () => {
    console.log(`[server] Web UI and API listening on http://${config.localIp}:${config.httpPort}`);
    console.log(`[server] Audio stream at http://${config.localIp}:${config.httpPort}/audio/stream`);
  });

  // Start the multiplexer (discovery + shairport)
  await multiplexer.start();

  // Graceful shutdown
  const shutdown = async (signal: string) => {
    console.log(`\n[main] Received ${signal}, shutting down...`);
    await multiplexer.stop();
    server.close(() => {
      console.log('[main] HTTP server closed');
      process.exit(0);
    });

    // Force exit after 10 seconds
    setTimeout(() => {
      console.error('[main] Forced exit after timeout');
      process.exit(1);
    }, 10000).unref();
  };

  process.on('SIGTERM', () => shutdown('SIGTERM'));
  process.on('SIGINT', () => shutdown('SIGINT'));
}

main().catch((err) => {
  console.error('[main] Fatal error:', err);
  process.exit(1);
});
