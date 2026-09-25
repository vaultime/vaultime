// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

// Supabase Edge Function: stripe-webhook
//
// Receives Stripe webhook events and updates the `subscriptions` table.
// Must be configured with the service role key so it can bypass RLS.
//
// Deploy with:
//   supabase functions deploy stripe-webhook --no-verify-jwt
//
// Required secrets (set via supabase secrets set):
//   STRIPE_SECRET_KEY
//   STRIPE_WEBHOOK_SECRET
//   SUPABASE_SERVICE_ROLE_KEY

import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";
import Stripe from "https://esm.sh/stripe@14?target=deno";

const stripe = new Stripe(Deno.env.get("STRIPE_SECRET_KEY")!, {
  apiVersion: "2024-06-20",
});

const webhookSecret = Deno.env.get("STRIPE_WEBHOOK_SECRET")!;

// Use service role to bypass RLS on the subscriptions table.
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

async function upsertSubscription(subscription: Stripe.Subscription) {
  const customerId =
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer.id;

  // Look up the Supabase user ID from customer metadata or existing row.
  let userId: string | null = null;

  const customer = await stripe.customers.retrieve(customerId);
  if (!customer.deleted) {
    userId = customer.metadata?.supabase_user_id ?? null;
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

  const row = {
    id: subscription.id,
    user_id: userId,
    tier: "pro",
    status: mapStatus(subscription.status),
    stripe_customer_id: customerId,
    stripe_subscription_id: subscription.id,
    current_period_start: new Date(
      subscription.current_period_start * 1000,
    ).toISOString(),
    current_period_end: new Date(
      subscription.current_period_end * 1000,
    ).toISOString(),
    cancel_at_period_end: subscription.cancel_at_period_end,
    updated_at: new Date().toISOString(),
  };

  const { error } = await supabase.from("subscriptions").upsert(row, {
    onConflict: "user_id",
  });

  if (error) {
    console.error("Failed to upsert subscription:", error);
  }
}

async function handleSubscriptionDeleted(subscription: Stripe.Subscription) {
  const customerId =
    typeof subscription.customer === "string"
      ? subscription.customer
      : subscription.customer.id;

  const { error } = await supabase
    .from("subscriptions")
    .update({
      status: "expired",
      tier: "free",
      updated_at: new Date().toISOString(),
    })
    .eq("stripe_customer_id", customerId);

  if (error) {
    console.error("Failed to expire subscription:", error);
  }
}

serve(async (req) => {
  const body = await req.text();
  const sig = req.headers.get("stripe-signature")!;

  let event: Stripe.Event;
  try {
    event = stripe.webhooks.constructEvent(body, sig, webhookSecret);
  } catch (err) {
    return new Response(JSON.stringify({ error: `Webhook error: ${err}` }), {
      status: 400,
    });
  }

  switch (event.type) {
    case "checkout.session.completed": {
      const session = event.data.object as Stripe.Checkout.Session;
      if (session.subscription) {
        const sub = await stripe.subscriptions.retrieve(
          session.subscription as string,
        );
        await upsertSubscription(sub);
      }
      break;
    }
    case "customer.subscription.created":
    case "customer.subscription.updated":
      await upsertSubscription(event.data.object as Stripe.Subscription);
      break;
    case "customer.subscription.deleted":
      await handleSubscriptionDeleted(event.data.object as Stripe.Subscription);
      break;
    default:
      break;
  }

  return new Response(JSON.stringify({ received: true }), { status: 200 });
});
