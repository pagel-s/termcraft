"""Offline contract tests; never call a model API."""
import json
import unittest
from generate_world import build_request, extract_recipe


class ProviderContracts(unittest.TestCase):
    def test_director_uses_current_session_context_and_smaller_output_budget(self):
        request = {"task": "director", "schema": {"type": "object"},
                   "prompt": "Give me a hard battle", "context": {"health": 20},
                   "instructions": "Change the current world with validated actions."}
        _, openai = build_request("openai", "configured-model", request)
        self.assertIn("current Termcraft game", openai["instructions"])
        self.assertEqual(openai["max_output_tokens"], 4000)
        self.assertEqual(json.loads(openai["input"])["context"]["health"], 20)
        _, anthropic = build_request("anthropic", "configured-model", request)
        self.assertEqual(anthropic["max_tokens"], 4000)
        self.assertNotIn("Match the player's theme using terrain", anthropic["system"])

    def test_requests_include_schema_and_repair_feedback(self):
        request = {"schema": {"type": "object", "properties": {"version": {"const": 1}}},
                   "prompt": "snowy ruins", "instructions": "Keep spawn clear.",
                   "previous_design": {"version": 9}, "validation_error": "wrong version"}
        _, openai = build_request("openai", "configured-model", request)
        self.assertEqual(openai["text"]["format"]["schema"]["properties"]["version"], {"enum": [1]})
        self.assertEqual(json.loads(openai["input"])["validation_error"], "wrong version")
        _, anthropic = build_request("anthropic", "configured-model", request)
        self.assertEqual(anthropic["tools"][0]["name"], "submit_world")
        self.assertEqual(anthropic["tool_choice"]["type"], "auto")
        self.assertEqual(request["schema"]["properties"]["version"], {"const": 1})

    def test_success_refusal_and_incomplete_responses(self):
        recipe = {"version": 1}
        response = {"status": "completed", "output": [{"type": "message", "content": [
            {"type": "output_text", "text": json.dumps(recipe)}]}]}
        self.assertEqual(extract_recipe("openai", response), recipe)
        with self.assertRaises(ValueError):
            extract_recipe("openai", {"status": "incomplete"})
        with self.assertRaises(ValueError):
            extract_recipe("openai", {"status": "completed", "output": [{"content": [{"type": "refusal"}]}]})
        response = {"stop_reason": "tool_use", "content": [
            {"type": "tool_use", "name": "submit_world", "input": recipe}]}
        self.assertEqual(extract_recipe("anthropic", response), recipe)
        response["content"].append(response["content"][0])
        with self.assertRaises(ValueError):
            extract_recipe("anthropic", response)
        with self.assertRaises(ValueError):
            extract_recipe("anthropic", {"stop_reason": "max_tokens", "content": []})


if __name__ == "__main__":
    unittest.main()
