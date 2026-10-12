#!/usr/bin/env python3
"""Optional Termcraft world designer/game master using Python's standard library.

The game owns validation, bounded repair attempts and saving. This runner makes
one provider request per invocation and returns a declarative recipe or action plan.
"""
import copy
import json
import os
import sys
import urllib.error
import urllib.request

MAX_BYTES = 2 * 1024 * 1024


def compatible_schema(schema):
    """Represent constants as enums for providers' JSON Schema subsets."""
    if isinstance(schema, dict):
        result = {key: compatible_schema(value) for key, value in schema.items()}
        if "const" in result:
            result["enum"] = [result.pop("const")]
        return result
    if isinstance(schema, list):
        return [compatible_schema(value) for value in schema]
    return schema


def build_request(provider, model, request):
    schema = compatible_schema(request["schema"])
    context = copy.deepcopy(request)
    del context["schema"]
    director = request.get("task") == "director"
    instructions = ("You direct the player's current Termcraft game. " + request["instructions"]
                    if director else (
        "You design playable Termcraft worlds. " + request["instructions"]
        + " Match the player's theme using terrain, regions and landmarks. "
        "Treat previous_design and validation_error as repair feedback. "
        "Never invent unsupported blocks, dimensions, entities, scripts or game rules."
    ))
    user = json.dumps(context, ensure_ascii=False)
    if provider == "openai":
        return "https://api.openai.com/v1/responses", {
            "model": model,
            "instructions": instructions,
            "input": user,
            "max_output_tokens": 4000 if director else 12000,
            "text": {"format": {"type": "json_schema", "name": "termcraft_world",
                                 "strict": True, "schema": schema}},
        }
    if provider == "anthropic":
        return "https://api.anthropic.com/v1/messages", {
            "model": model,
            "max_tokens": 4000 if director else 12000,
            "system": instructions + " Submit exactly one complete design using submit_world.",
            "messages": [{"role": "user", "content": user}],
            "tools": [{"name": "submit_world", "description": "Submit a declarative world design.",
                       "input_schema": schema}],
            "tool_choice": {"type": "auto", "disable_parallel_tool_use": True},
        }
    raise ValueError("TERMCRAFT_MODEL_PROVIDER must be openai or anthropic")


def extract_recipe(provider, response):
    if provider == "openai":
        if response.get("status") != "completed":
            raise ValueError("provider response was not completed; no design accepted")
        texts = []
        for item in response.get("output", []):
            for block in item.get("content", []):
                if block.get("type") == "refusal":
                    raise ValueError("provider declined to generate this design")
                if block.get("type") == "output_text":
                    texts.append(block["text"])
        if not texts:
            raise ValueError("provider returned no world design")
        return json.loads("".join(texts))
    if response.get("stop_reason") != "tool_use":
        raise ValueError("provider did not submit a complete world design")
    designs = [block["input"] for block in response.get("content", [])
               if block.get("type") == "tool_use" and block.get("name") == "submit_world"]
    if len(designs) != 1:
        raise ValueError("provider must submit exactly one world design")
    return designs[0]


def main():
    provider = os.environ.get("TERMCRAFT_MODEL_PROVIDER", "")
    model = os.environ.get("TERMCRAFT_MODEL", "")
    if not model:
        raise ValueError("set TERMCRAFT_MODEL to a model supported by your provider")
    key_name = {"openai": "OPENAI_API_KEY", "anthropic": "ANTHROPIC_API_KEY"}.get(provider)
    if not key_name:
        raise ValueError("set TERMCRAFT_MODEL_PROVIDER to openai or anthropic")
    key = os.environ.get(key_name)
    if not key:
        raise ValueError("set " + key_name + " for the selected provider")
    raw = sys.stdin.buffer.read(512 * 1024 + 1)
    if len(raw) > 512 * 1024:
        raise ValueError("world design request is too large")
    request = json.loads(raw)
    if request.get("protocol") != 1:
        raise ValueError("unsupported world generator protocol")
    url, body = build_request(provider, model, request)
    headers = {"Content-Type": "application/json"}
    if provider == "openai":
        headers["Authorization"] = "Bearer " + key
    else:
        headers.update({"x-api-key": key, "anthropic-version": "2023-06-01"})
    http_request = urllib.request.Request(url, json.dumps(body).encode(), headers, method="POST")
    # No automatic retries: the game retries invalid designs, never transport errors.
    # Disable redirects so credentials cannot follow a provider redirect elsewhere.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            return None
    with urllib.request.build_opener(NoRedirect).open(http_request, timeout=60) as response:
        raw = response.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError("provider response is too large")
    recipe = extract_recipe(provider, json.loads(raw))
    if not isinstance(recipe, dict):
        raise ValueError("world design must be a JSON object")
    print(json.dumps(recipe, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except urllib.error.HTTPError as error:
        # Do not print request headers, keys, or arbitrary provider error bodies.
        print(f"World provider HTTP {error.code}; check model, credentials and quota.", file=sys.stderr)
        sys.exit(1)
    except (ValueError, KeyError, OSError, urllib.error.URLError) as error:
        print(f"World generation failed: {error}", file=sys.stderr)
        sys.exit(1)
