// The simulation and renderer run off the UI thread. No server processes or sockets.
import init, { BrowserGame } from './pkg/termcraft.js?v=natural-world-1';

const ready = init({module_or_path:new URL('./pkg/termcraft_bg.wasm?v=natural-world-1',import.meta.url)});
let game;
self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    await ready;
    let result;
    switch (type) {
      case 'start':
        game?.free();
        game = undefined;
        game = new BrowserGame(data.columns, data.rows, data.saved ?? undefined, data.mode);
        result = JSON.parse(game.status());
        break;
      case 'frame': {
        const cells = game.frame(data.elapsed);
        self.postMessage({ id, type, cells, status: JSON.parse(game.status()) }, [cells.buffer]);
        return;
      }
      case 'key': result = game.key(data.code, data.down, data.shift); break;
      case 'aim': game.aim(data.x, data.y); break;
      case 'mouse': game.mouse(data.x, data.y, data.button, data.phase, data.shift); break;
      case 'wheel': game.wheel(data.down); break;
      case 'release': game?.release_controls(); break;
      case 'save': result = game.snapshot(); break;
      default: throw new Error('Unknown game request.');
    }
    if (id) self.postMessage({ id, type, result });
  } catch (error) {
    self.postMessage({ id, type: 'error', message: String(error?.message ?? error) });
  }
};
