package com.example.shop.order;

import com.example.shop.domain.Money;

/**
 * Charges customers. Implementations talk to the payment provider.
 */
public interface PaymentGateway {

    /** Returns the provider's transaction id. */
    String charge(String customerId, Money amount) throws PaymentException;

    void refund(String transactionId, Money amount) throws PaymentException;

    class PaymentException extends Exception {
        public PaymentException(String message) {
            super(message);
        }

        public PaymentException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
