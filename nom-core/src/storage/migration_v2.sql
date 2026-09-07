-- Migration v2: add meals.meal_type (breakfast/lunch/dinner).
-- Appended as the LAST column so fresh installs (v1 then v2) and upgraded
-- installs (v2 only) converge on identical column order. NULL is intentional
-- for pre-existing rows — SQL CHECK is tri-valued, so NULL passes.
ALTER TABLE meals ADD COLUMN meal_type TEXT CHECK (meal_type IN ('breakfast','lunch','dinner'));
