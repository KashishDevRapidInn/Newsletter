UPDATE subscriptions 
SET status = 'confirmed'
WHERE status IS NULL;