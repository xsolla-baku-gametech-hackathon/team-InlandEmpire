"""Pay one Xsolla sandbox checkout with the test card, headless.

Usage: python3 scripts/autopay.py <checkout_url>

Stands in for Xsolla Tokenization, which needs partner approval. Sandbox only.
Needs: pip install playwright && playwright install chromium
"""

import sys
import time

from playwright.sync_api import sync_playwright

TEST_CARD = [
    ("card_number", "4111111111111111"),
    ("card_month", "1240"),
    ("cvv", "123"),
    ("email", "tappad@example.com"),
    ("zip", "12345"),
]
DONE_WORDS = ("success", "thank", "paid", "fail", "declin", "error")


def pay(url: str) -> str:
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 900, "height": 1200})
        page.goto(url, wait_until="networkidle", timeout=60000)
        time.sleep(3)
        page.get_by_text("Card", exact=True).first.click()
        time.sleep(2)
        for name, value in TEST_CARD:
            page.click(f"input[name={name}]")
            page.keyboard.type(value, delay=40)
        time.sleep(3)
        page.locator("button", has_text="Pay US$").first.click()
        text = ""
        for _ in range(40):
            time.sleep(1)
            text = page.evaluate("document.body.innerText")
            if any(w in text.lower() for w in DONE_WORDS):
                break
        browser.close()
        return text


if __name__ == "__main__":
    if len(sys.argv) != 2 or "sandbox" not in sys.argv[1]:
        sys.exit("usage: autopay.py <sandbox checkout url>")
    result = pay(sys.argv[1])
    ok = "success" in result.lower() or "thank" in result.lower()
    print("autopay", "ok" if ok else "unclear", result[:200].replace("\n", " "))
    sys.exit(0 if ok else 1)
