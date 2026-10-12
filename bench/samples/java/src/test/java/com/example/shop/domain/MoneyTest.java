package com.example.shop.domain;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Nested;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.CsvSource;

import java.math.BigDecimal;
import java.util.Currency;

import static org.assertj.core.api.Assertions.assertThat;
import static org.junit.jupiter.api.Assertions.assertAll;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class MoneyTest {

    private static final Currency EUR = Currency.getInstance("EUR");

    @ParameterizedTest
    @CsvSource({"1.005, 1.00", "1.015, 1.02", "0, 0.00"})
    void rounds_half_even_to_the_currency_scale(String input, String expected) {
        assertThat(new Money(new BigDecimal(input), EUR).amount()).isEqualByComparingTo(expected);
    }

    @Test
    void adds_amounts_of_the_same_currency() {
        Money sum = Money.of("1.50", "EUR").plus(Money.of("2.25", "EUR"));
        assertAll(
                () -> assertThat(sum.amount()).isEqualByComparingTo("3.75"),
                () -> assertThat(sum.currency()).isEqualTo(EUR));
    }

    @Test
    void refuses_to_add_different_currencies() {
        Money euros = Money.of("1.00", "EUR");
        Money dollars = Money.of("1.00", "USD");
        assertThrows(IllegalArgumentException.class, () -> euros.plus(dollars));
    }

    @Test
    void zero_is_zero() {
        assertTrue(Money.zero(EUR).isZero());
    }

    @Nested
    class Percentages {

        @Test
        void computes_a_percentage() {
            assertThat(Money.of("200.00", "EUR").percent(new BigDecimal("7.5")).amount())
                    .isEqualByComparingTo("15.00");
        }

        @Disabled("JDK-8290000: HALF_EVEN differs on some JDK 21 builds; re-enable after the upgrade")
        @Test
        void negative_percentages_round_towards_even() {
            assertThat(Money.of("0.25", "EUR").percent(new BigDecimal("-10")).amount())
                    .isEqualByComparingTo("-0.02");
        }
    }
}
