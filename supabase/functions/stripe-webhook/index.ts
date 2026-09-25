// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { createClient } from "https://esm.sh/@supabase/supabase-js@2";
import Stripe from "https://esm.sh/stripe@17?target=deno";

const stripe = new Stripe(Deno.env.get("STRIPE_SECRET_KEY")!);

const webhookSecret = Deno.env.get("STRIPE_WEBHOOK_SECRET")!;

const supabase = createClient(
  Deno.env.get("SUPABASE_URL")!,
  Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!,
);

function mapStatus(stripeStatus: string): string {
  switch (stripeStatus) {
    case "active":
      return "active";
    case "past_due":
      return "past_due";
    case "canceled":
      return "canceled";
    case "unpaid":
    case "incomplete_expired":
      return "expired";
    default:
      return "none";
  }
}

// deno-lint-ignore no-explicit-any
async function upsertSubscription(subscription: any) {
  const customerId =
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer?.id;

  if (!customerId) {
    console.error("No customer ID in subscription object");
    return;
  }

  let userId: string | null = null;

  try {
    const customer = await stripe.customers.retrieve(customerId);
    if (customer && !customer.deleted) {
      userId = customer.metadata?.supabase_user_id ?? null;
    }
  } catch (err) {
    console.error("Failed to retrieve Stripe customer:", err);
  }

  if (!userId) {
    const { data: existing } = await supabase
      .from("subscriptions")
      .select("user_id")
      .eq("stripe_customer_id", customerId)
      .single();
    userId = existing?.user_id ?? null;
  }

  if (!userId) {
    console.error(
      `Cannot resolve Supabase user for Stripe customer ${customerId}`,
    );
    return;
  }

  const periodStart = subscription.current_period_start;
  const periodEnd = subscription.current_period_end;

  const row = {
    id: subscription.id,
    user_id: userId,
    tier: "pro",
    status: mapStatus(subscription.status ?? "none"),
    stripe_customer_id: customerId,
    stripe_subscription_id: subscription.id,
    current_period_start: periodStart
      ? new Date(typeof periodStart === "number" ? periodStart * 1000 : periodStart).toISOString()
      : null,
    current_period_end: periodEnd
      ? new Date(typeof periodEnd === "number" ? periodEnd * 1000 : periodEnd).toISOString()
      : null,
    cancel_at_period_end: subscription.cancel_at_period_end ?? false,
    updated_at: new Date().toISOString(),
  };

  console.log("Upserting subscription row:", JSON.stringify(row));

  const { error } = await supabase.from("subscriptions").upsert(row, {
    onConflict: "user_id",
  });

  if (error) {
    console.error("Failed to upsert subscription:", JSON.stringify(error));
  }
}

// deno-lint-ignore no-explicit-any
async function handleSubscriptionDeleted(subscription: any) {
  const customerId =
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer?.id;

  const { error } = await supabase
    .from("subscriptions")
    .update({
      status: "expired",
      tier: "free",
      updated_at: new Date().toISOString(),
    })
    .eq("stripe_customer_id", customerId);

  if (error) {
    console.error("Failed to expire subscription:", JSON.stringify(error));
  }
}

Deno.serve(async (req) => {
  const body = await req.text();
  const sig = req.headers.get("stripe-signature");

  if (!sig) {
    return new Response(JSON.stringify({ error: "Missing stripe-signature" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  // deno-lint-ignore no-explicit-any
  let event: any;
  try {
    event = await stripe.webhooks.constructEventAsync(body, sig, webhookSecret);
  } catch (err) {
    console.error("Webhook signature verification failed:", err);
    return new Response(JSON.stringify({ error: `Webhook error: ${err}` }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  console.log("Received Stripe event:", event.type, event.id);

  try {
    switch (event.type) {
      case "checkout.session.completed": {
        const session = event.data.object;
        if (session.subscription) {
          const subId =
            typeof session.subscription === "string"
              ? session.subscription
              : session.subscription.id;
          const sub = await stripe.subscriptions.retrieve(subId);
          await upsertSubscription(sub);
        }
        break;
      }
      case "customer.subscription.created":
      case "customer.subscription.updated":
        await upsertSubscription(event.data.object);
        break;
      case "customer.subscription.deleted":
        await handleSubscriptionDeleted(event.data.object);
        break;
      default:
        console.log("Unhandled event type:", event.type);
        break;
    }
  } catch (err) {
    console.error("Error processing webhook event:", err);
    return new Response(JSON.stringify({ error: `Processing error: ${err}` }), {
      status: 500,
      headers: { "Content-Type": "application/json" },
    });
  }

  return new Response(JSON.stringify({ received: true }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
