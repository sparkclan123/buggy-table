import type { Page } from '@playwright/test';

// Editing intentionally requires selecting the row before activating its cell.
export async function editCell(page: Page, row: number, column: string) {
  await page.getByRole('button', { name: `Select row ${row}`, exact: true }).click();
  await page.getByRole('button', { name: `Edit row ${row} ${column}`, exact: true }).click();
}
