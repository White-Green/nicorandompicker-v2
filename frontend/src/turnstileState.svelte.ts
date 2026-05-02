let turnstileRecommendedState = $state(false);

export function turnstileRecommended(): boolean {
  return turnstileRecommendedState;
}

export function setTurnstileRecommended(value: boolean): void {
  turnstileRecommendedState = value;
}

export function recommendTurnstile(): void {
  turnstileRecommendedState = true;
}

export function clearTurnstileRecommendation(): void {
  turnstileRecommendedState = false;
}
