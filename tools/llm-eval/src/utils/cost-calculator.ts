/**
 * Token cost calculation for various LLM models.
 */

interface ModelPricing {
  readonly inputPerMillion: number;
  readonly outputPerMillion: number;
}

const PRICING: Record<string, ModelPricing> = {
  // Anthropic
  "claude-opus-4-5-20251101": { inputPerMillion: 15.0, outputPerMillion: 75.0 },
  "claude-sonnet-4-5-20250929": { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  "claude-haiku-3-5-20241022": { inputPerMillion: 0.8, outputPerMillion: 4.0 },
  // OpenAI
  "gpt-4o": { inputPerMillion: 2.5, outputPerMillion: 10.0 },
  "gpt-4o-mini": { inputPerMillion: 0.15, outputPerMillion: 0.6 },
  "o1": { inputPerMillion: 15.0, outputPerMillion: 60.0 },
  "o1-mini": { inputPerMillion: 1.1, outputPerMillion: 4.4 },
};

/** Default pricing when model is not in the lookup table. */
const DEFAULT_PRICING: ModelPricing = { inputPerMillion: 3.0, outputPerMillion: 15.0 };

/**
 * Calculate the USD cost for a generation call.
 */
export function calculateCost(
  modelId: string,
  promptTokens: number,
  completionTokens: number,
): number {
  const pricing = PRICING[modelId] ?? DEFAULT_PRICING;
  return (
    (promptTokens / 1_000_000) * pricing.inputPerMillion +
    (completionTokens / 1_000_000) * pricing.outputPerMillion
  );
}
