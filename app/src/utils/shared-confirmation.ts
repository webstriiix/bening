import { Connection } from "@solana/web3.js";
import type { RpcResponseAndContext, SignatureStatus } from "@solana/web3.js";
import { parseAnchorError } from "./errors";

/**
 * Poll getSignatureStatuses until confirmed/finalized or timeout.
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

    const statusResult = statuses as unknown as RpcResponseAndContext<(SignatureStatus | null)[]>;
    const status = statusResult.value[0];
    if (!status) {
      await new Promise((r) => setTimeout(r, intervalMs));
      continue;
    }

    if (status.err) {
      throw new Error(
        `Transaction failed: ${status.err instanceof Error ? status.err.message : String(status.err)}`
      );
    }

    if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
      return;
    }

    await new Promise((r) => setTimeout(r, intervalMs));
  }

  const explorerLink = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
  throw new Error(`Not confirmed yet; check explorer ${explorerLink}`);
}

/**
 * Map program errors from IDL to plain English explanations.
 *
 * @param err The error from a Solana transaction
 * @returns Plain English error message
 */
export function mapProgramError(err: any): string {
  try {
    return parseAnchorError(err);
  } catch (e) {
    return err instanceof Error ? err.message : String(err);
  }
}

/**
 * Poll for confirmation and map any errors encountered.
 * Intended to be used by transaction wrappers.
 *
 * @param connection Solana connection
 * @param signature Transaction signature
 * @param errorMapper Optional custom error mapper
 * @returns Promise<void>
 */
export async function pollForConfirmation(
  connection: Connection,
  signature: string,
  errorMapper?: (err: any) => string
): Promise<void> {
  const timeoutMs = 90000;
  const intervalMs = 1500;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    const statuses = await connection.getSignatureStatuses(
      [signature],
      { searchTransactionHistory: true }
    );

    const statusResult = statuses as unknown as RpcResponseAndContext<(SignatureStatus | null)[]>;
    const status = statusResult.value[0];
    if (!status) {
      await new Promise((r) => setTimeout(r, intervalMs));
      continue;
    }

    if (status.err) {
      const errorMsg = errorMapper ? errorMapper(status.err) : mapProgramError(status.err);
      throw new Error(`Transaction failed: ${errorMsg}`);
    }

    if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
      return;
    }

    await new Promise((r) => setTimeout(r, intervalMs));
  }

  const explorerLink = `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
  throw new Error(`Not confirmed yet; check explorer ${explorerLink}`);
}