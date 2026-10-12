import java.util.List;
import java.util.function.Predicate;
import org.example.shop.Ticket;
import org.example.shop.TicketService;

// Receivers whose type the resolver cannot know: calls into library classes, inherited
// fields, lambda parameters. A string literal argument makes the swap safe for any string.
class Dispatcher extends BaseDispatcher {
    private TicketService service;

    <T> void route(Ticket ticket, T token, Ticket[] batch, String[] names, String... flags) {
        if (ticket.getStatus().equals("OPEN")) {                   // expect: literal-first-in-equals
            open(ticket);
        }
        boolean urgent = service.priorityOf(ticket).equalsIgnoreCase("high"); // expect: literal-first-in-equals
        boolean mine = inheritedOwner.equals("me");                 // expect: literal-first-in-equals
        Predicate<Ticket> p = t -> t.getQueue().equals("support");  // expect: literal-first-in-equals
        List<Ticket> all = service.all();
        all.removeIf(t -> t.getLabel().equals("spam"));            // expect: literal-first-in-equals
        var code = service.codeOf(ticket);
        boolean zero = code.equals("0");                            // expect: literal-first-in-equals
        boolean first = names[0].equals("a");                       // expect: literal-first-in-equals

        // Not reported.
        boolean n1 = "OPEN".equals(ticket.getStatus());
        boolean n2 = ticket.equals("OPEN");                         // declared as Ticket: not a string
        boolean n3 = token.equals("x");                             // type variable, not String
        boolean n4 = batch[0].equals("x");                          // element of a Ticket[]
        boolean n5 = names.equals("x");                             // the array itself
        boolean n6 = flags.equals("x");                             // varargs array
        boolean n7 = ticket.getStatus().equals(OPEN_STATE);         // expect: literal-first-in-equals
        boolean n8 = ticket.getStatus() != null && ticket.getStatus().equals("OPEN");
        boolean n9 = ticket.getStatus().trim().equals("OPEN");      // trim() already dereferences it
        boolean n10 = service.nameOf(ticket).toString().equals("x");
        boolean n11 = Ticket.DEFAULT_QUEUE.equals("x");             // constant receiver
        boolean n12 = getClass().getName().equals("Dispatcher");
        boolean n13 = ticket.getStatus().equals("OPEN", "CLOSED");  // other arity
        // Only the first call is reported: the right side's receiver was dereferenced on the left.
        // expect-next-line: literal-first-in-equals
        boolean n14 = ticket.getStatus().equals("A") || ticket
                .getStatus().equals("B");
        if (inheritedOwner == null || inheritedOwner.equals("nobody")) {
            return;
        }
    }

    private static final String OPEN_STATE = "OPEN";

    void open(Ticket t) {
    }
}
