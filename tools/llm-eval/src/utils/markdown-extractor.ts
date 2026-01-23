/**
 * Extracts Covenant code from markdown-formatted LLM responses.
 */

/**
 * Extract code from markdown code blocks.
 * Handles ```covenant, ```cov, and bare ``` fences.
 * Returns the raw text if no fences are found.
 */
export function extractCodeFromMarkdown(text: string): string {
  const pattern = /```(?:covenant|cov)?\n([\s\S]*?)```/;
  const match = text.match(pattern);
  if (match?.[1]) {
    return match[1].trim();
  }
  return text.trim();
}
