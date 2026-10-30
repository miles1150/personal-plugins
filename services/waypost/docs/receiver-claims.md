# Receiver processing claims

Use `waypost_receiver_claim` and `waypost_receiver_complete` when multiple workers, webhook invocations, or scheduled runs may process the same message. All cooperating workers must use the same Actor and Consumer identity. The server coordinates claims by `(space_id, actor_id, consumer_id, item_id)`; a different Consumer is a different processing workflow.

Claims apply to visible message feed items. Use the feed `item_id`, not the message resource ID. A claim does not acknowledge receipt or processing, start a worker, notify a host, or execute a business action.

1. Read the message with your own authenticated connection. Persist the full message and the pending work in a durable local ledger. Emit the existing `received` receipt only after that save succeeds.
2. Create and persist a fresh `attempt_id` for this execution attempt, then call:

   ```json
   {"item_id":"feed_example","attempt_id":"worker-run-123","lease_seconds":60}
   ```

   The lease defaults to 60 seconds and must be between 1 and 300 seconds. The response includes `acquired`, `status`, `generation`, `lease_expires_at` (Unix milliseconds), and `request_id`.
3. Proceed only when `acquired=true` and `status=claimed`. Persist the returned generation and request ID. Other workers get `acquired=false`. A retry with the same attempt ID does not renew the lease. Its original grant or denial is retained, including after another worker takes over. Reusing that ID with a different lease returns `IDEMPOTENCY_CONFLICT`.
4. Carry out the user-authorized action within the lease. Use the returned `request_id` as the stable business idempotency key when sending a Waypost reply or calling another service that supports idempotency. Persist the exact action parameters and result. The request ID stays the same across attempts and generations for this item and identity; the attempt ID identifies only one lease request.
5. After actual processing succeeds, call `waypost_receiver_complete`:

   ```json
   {"item_id":"feed_example","attempt_id":"worker-run-123","generation":1,"request_id":"receiver:SERVER_RETURNED_HASH"}
   ```

   Completion checks the current unexpired generation and atomically records the normal `processed` receipt. The result uses the existing acknowledgment shape with `submission_state=committed` and `items[].stage=processed`. Retrying the same completed generation returns the same receipt result, including after its original lease expires.

When a lease expires, stop this attempt. A new attempt ID can claim the item with a strictly higher generation. The old worker cannot complete the new generation: the server compares the generation and attempt ID inside the receipt transaction. `REVISION_CONFLICT` means the claim expired, was superseded, or is otherwise no longer claimable. A successful claim never emits `received` or `processed` by itself.

Reconcile uncertain business results before acting again. A process can perform an external action and crash before completion, or continue running after its lease expires. The generation fences Waypost completion; external systems must enforce the stable business idempotency key or their own fence to prevent duplicate effects. Claims do not promise exactly-once execution. Operations that cannot finish within the bounded lease need a separate durable execution design; this protocol does not provide lease renewal.

Completed claims are terminal. Legacy `waypost_ack` and the existing receipt HTTP endpoint remain compatible and can still publish `processed` without a claim. Those clients do not gain concurrency protection automatically. Upgrade every processing entry point for a shared identity before relying on claims to coordinate it; bridge notification acceptance and a `received` receipt still provide no processing lock.

Keep existing credentials, Actor/Consumer identities, local ledgers, cursors, and receipt history when adopting this workflow. The additive migration creates claim and attempt tables and does not rewrite message or receipt data.
