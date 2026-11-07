import { Connection } from "@solana/web3.js";
import type { RpcResponseAndContext, SignatureStatus } from "@solana/web3.js";

/**
 * Polls getSignatureStatuses until the signature is confirmed/finalized or times out.
 *
 * @param connection Solana connection to query
 * @param signature Transaction signature to check
 * @param options Polling options
 * @param options.timeoutMs Maximum time in ms to poll (default: 90000)
 * @param options.intervalMs Time between polls in ms (default: 1500)
 * @returns Promise<void> Resolves when confirmed/finalized
 * @throws {Error} If status.err is set, or if timeout is reached
 */
export async function sendAndConfirmPolling(
  connection: Connection,
  signature: string,
  options?: { timeoutMs?: number; intervalMs?: number }
): Promise<void> {
  const timeoutMs = options?.timeoutMs ?? 90000;
  const intervalMs = options?.intervalMs ?? 1500;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    const statuses = await connection.getSignatureStatuses(
      [signature],
      { searchTransactionHistory: true }
    );

    // Extract status safely - use destructuring to satisfy TS
    const statusResult = statuses as unknown as RpcResponseAndContext<(SignatureStatus | null)[]>;
    const status = statusResult.value[0];
    if (!status) {
      await new Promise((r) => setTimeout(r, intervalMs));
      continue;
    }

    // If there's an error, throw immediately
    if (status.err) {
      throw new Error(
        `Transaction failed: ${status.err instanceof Error ? status.err.message : String(status.err)}`
      );
    }

    // If confirmed or finalized, success
    if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
      return;
    }

    // Otherwise, wait and poll again
    await new Promise((r) => setTimeout(r, intervalMs));
  }

  // Timeout reached
  const explorerLink = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
  throw new Error(
    `Not confirmed yet; check explorer ${explorerLink}`
  );
}