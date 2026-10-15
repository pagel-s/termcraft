#!/usr/bin/env python3
"""Serve both Termcraft review galleries locally, with fresh HTML after updates."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class GalleryHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/':
            self.send_response(302)
            self.send_header('Location', '/world-atlas-2026-10-15/')
            self.end_headers()
            return
        super().do_GET()

    def end_headers(self):
        # Images carry content revisions in their URLs; always refresh the HTML
        # and catalog so an existing tab sees the latest captures after reload.
        self.send_header('Cache-Control', 'no-cache')
        super().end_headers()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8774)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'docs/design'
    handler = partial(GalleryHandler, directory=str(root))
    server = ThreadingHTTPServer(('127.0.0.1', args.port), handler)
    print(f'http://127.0.0.1:{server.server_port}/', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == '__main__':
    main()
