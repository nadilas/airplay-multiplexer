import { Router, Request, Response } from 'express';

// The multiplexer instance is injected
export function createApiRouter(getMultiplexer: () => any): Router {
  const router = Router();

  router.get('/status', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    res.json(multiplexer.getStatus());
  });

  router.get('/devices', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    const status = multiplexer.getStatus();
    res.json(status.devices);
  });

  router.post('/devices/:id/volume', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    const { id } = req.params;
    const { volume } = req.body;

    if (typeof volume !== 'number' || volume < 0 || volume > 100) {
      return res.status(400).json({ error: 'Volume must be a number between 0 and 100' });
    }

    multiplexer
      .setDeviceVolume(id, volume)
      .then(() => res.json({ ok: true }))
      .catch((err: Error) => res.status(404).json({ error: err.message }));
  });

  router.post('/devices/:id/mute', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    const { id } = req.params;
    const { muted } = req.body;

    if (typeof muted !== 'boolean') {
      return res.status(400).json({ error: 'muted must be a boolean' });
    }

    multiplexer
      .setDeviceMute(id, muted)
      .then(() => res.json({ ok: true }))
      .catch((err: Error) => res.status(404).json({ error: err.message }));
  });

  router.post('/devices/:id/enable', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    const { id } = req.params;
    const { enabled } = req.body;

    if (typeof enabled !== 'boolean') {
      return res.status(400).json({ error: 'enabled must be a boolean' });
    }

    multiplexer
      .setDeviceEnabled(id, enabled)
      .then(() => res.json({ ok: true }))
      .catch((err: Error) => res.status(404).json({ error: err.message }));
  });

  router.post('/master-volume', (req: Request, res: Response) => {
    const multiplexer = getMultiplexer();
    const { volume } = req.body;

    if (typeof volume !== 'number' || volume < 0 || volume > 100) {
      return res.status(400).json({ error: 'Volume must be a number between 0 and 100' });
    }

    multiplexer
      .setMasterVolume(volume)
      .then(() => res.json({ ok: true }))
      .catch((err: Error) => res.status(500).json({ error: err.message }));
  });

  // Server-Sent Events for real-time updates
  router.get('/events', (req: Request, res: Response) => {
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    });

    res.write('data: {"type":"connected"}\n\n');

    const multiplexer = getMultiplexer();

    const onStatusChange = () => {
      const status = multiplexer.getStatus();
      res.write(`data: ${JSON.stringify({ type: 'status', ...status })}\n\n`);
    };

    multiplexer.on('statusChanged', onStatusChange);

    // Send initial status
    onStatusChange();

    // Heartbeat every 30s to keep connection alive
    const heartbeat = setInterval(() => {
      res.write(': heartbeat\n\n');
    }, 30000);

    req.on('close', () => {
      multiplexer.off('statusChanged', onStatusChange);
      clearInterval(heartbeat);
    });
  });

  return router;
}
