"""What the page and the manual say.

Prose lives here rather than inside the templates so that `build.py` stays a
builder and this stays a text. Anything that is a *fact about the program* —
the keys, the polar, the number of tests — is read out of the repository by
`build.py` instead of being written down here, because a fact written twice is
a fact that will disagree with itself.
"""

TAGLINE = "A sailing simulator in the spirit of a long-haul truck sim."

STANDFIRST = (
    "You skipper a small cruising sloop, and the point is the passage rather than "
    "the race. <strong>Apparent wind, sail trim, heel and righting moment, leeway, the "
    "hull-speed wall, rudder stall, weather helm</strong> — modelled for real, not a wind "
    "arrow and a speed multiplier."
)

# ---- the front page -------------------------------------------------------

WHAT_IT_IS = [
    (
        "The loop is the sailing",
        """True wind sets your trim; trim gives you drive and leeway; drive shifts the
        apparent wind; and you trim again. Everything else — anchoring, the auxiliary,
        winches, reefing — extends that loop rather than standing beside it.""",
    ),
    (
        "Angle of attack, not wind angle",
        """A velocity prediction program can assume optimal trim and tabulate against wind
        angle alone. A game cannot, because trimming is the player's job. The sheet sets
        the chord angle, the chord angle sets the angle of attack, and the angle of attack
        decides whether the sail drives, luffs, or stalls.""",
    ),
    (
        "It teaches you to sail",
        """The overboard drill is also the lesson: one instruction at a time, nothing until
        the last one has been done, and then the dan buoy goes over the side and the rest of
        it matters. Getting a boat going is three keys and two minutes; getting her back to
        one spot and <em>stopped</em> there is the whole of boat handling, and nobody
        practises it until they need it.""",
    ),
    (
        "One boat, modelled properly",
        """A production charter sloop of about 34 feet — the class most sailors have
        actually steered, and can therefore judge. One boat modelled properly beats five
        modelled vaguely.""",
    ),
]

MODEL_INTRO = """Every row here is arithmetic that can be wrong in a way a sailor would
notice, which is why the core carries a validation suite that asserts against figures
known from the water rather than against its own output."""

NAVIGATION = """The archipelago has names on it, and finding out which bit of it you are
looking at is a game in its own right — the oldest one there is, and the one a chart
plotter deleted."""

NAVIGATION_POINTS = [
    (
        "The chart is paper",
        """<code>M</code> puts it in the corner; <code>Shift&nbsp;M</code> spreads it on the
        table with the instruments a chart table has — dividers, a parallel rule, a
        protractor, a pencil, a stopwatch, a hand log, and a rubber. The coast on it is
        read off the same signed-distance field the boat runs aground on, so the chart
        cannot lie about the depths: there is only one set of them.""",
    ),
    (
        "Seeing a thing and knowing which thing it is",
        """A lighthouse is painted to be told apart and lit; a beacon has to be close
        enough for its topmark to be read, or it is a stick. The glasses roughly double the
        range at which a mark can be <em>named</em>, and three marks well spread apart are
        a shape that belongs in one place on the coast and nowhere else.""",
    ),
    (
        "A bearing is a line, not a position",
        """The hand bearing compass has a notch about a degree and a quarter wide, and the
        number carries the error a card in a bowl actually has — two or three degrees in a
        seaway. Which is why a fix is a triangle rather than a point, why a fine cut is a
        bad fix, and why the third line is worth taking.""",
    ),
    (
        "The running fix",
        """One mark, two bearings and the run between them are a position. Every position
        line is drawn through the mark plus the distance she has run since it was shot —
        fresh, it passes through the mark; twenty minutes later it is a hand's breadth off
        it, dashed. The oldest trick on a coast with a single light.""",
    ),
    (
        "Lost on purpose",
        """<code>Shift&nbsp;L</code> puts you somewhere at sea with no idea where. The chart
        stops following her, the names come off the marks, and getting them back is the
        exercise — scored on whether the fix is near where she actually is, not on whether
        the cocked hat was small. A tight triangle on two misidentified marks is a
        confident lie and the classic way to put a boat on a rock.""",
    ),
]

SOUND = """Synthesised, not sampled. There are no audio files in the repository and there
do not need to be: wind and water <em>are</em> filtered noise, so generating them is the
honest method rather than a shortcut. The rigging note comes from the Strouhal relation on
a 6&nbsp;mm shroud, and it is gated on how steady the wind is — vortex shedding cannot lock
on to a flow that is changing underneath it, so a shroud in a gust lets go, goes quiet, and
finds a new note when the gust steadies."""

SOUND_BAND = """There is a band as well, and it is written by the boat rather than played
at her. The tempo comes from the encounter period — the rate the hull actually meets the
waves at — so the music is in the same rhythm as the sailing instead of across it. Beating
is syncopated and square, reaching gets a walking bass, running is sparse. Past what the
boat can take, it changes language entirely, into something with a flattened second in
it."""

TOGETHER = """Two to eight friends in one archipelago. Every boat is computed by the person
sailing her and the server is a relay, a clock and a lobby — anything else would put a
round trip between somebody's hand and their rudder, and a sailor feels fifty milliseconds
on the tiller. A game on the same network is in the list, because an address read out over
a chat window is wrong about a third of the time."""

NOT_IN_THE_BOX = """<b>Not modelled yet, and known to be absent: tide and current.</b> The
chart draws a wind-driven set and drift in its corner, two and a half per cent of the wind,
and the sea's own texture is carried along by it — but the hull is not, so that arrow is
the one thing on the paper the boat does not obey. Worth knowing before trusting a
dead-reckoning position. Time runs 1:1 with no compression, which is a deliberate decision
and one that will have to be revisited: forty miles at five knots is eight hours."""

WHY_NOT_BROWSER = """<b>There is no demo you can play in this tab.</b> The whole thing
compiles for <code>wasm32-unknown-unknown</code>, so that is not what stops it — what stops
it is sailing together. A browser has no UDP and cannot broadcast, so the relay, the list of
games on the network and joining by address all go, and the replacement is a certificate and
a public host. The pictures on this page are what the program looks like; the program itself
is two lines below."""

# ---- the manual -----------------------------------------------------------

MANUAL_STANDFIRST = """Everything you need to get her off a mooring, through a passage and
into the next harbour — and what to do when it goes wrong. The key list in it is generated
from the program's own help screen, which is the only copy of it that cannot drift."""

MANUAL = [
    (
        "before-you-start",
        "Before you start",
        """<p>Nothing here needs an account, a launcher or a network. On a Mac or a PC it
        is one line in a terminal; from source it is one <code>cargo</code> command. The
        installation lines are on the front page and in the repository's README.</p>

        <p><b>Gamepad is the intended input, and a keyboard works.</b> The design rule is
        that you map <em>intentions</em> rather than lines, and that each control's nature
        matches the gear it stands for: the helm and the mainsheet are positions your hand
        holds, the genoa sheet is a winch you grind, and a reef is a job you do.</p>

        <p><b>The cockpit camera heels with the boat.</b> That is deliberate. Heel is the
        primary feedback channel on a sailing boat, and a view that stays level throws it
        away.</p>

        <p><b>The picture has a grain, and you can choose how coarse it is.</b>
        <code>Esc</code>, then <b>PICTURE</b>: the row marked <em>pixels</em> offers 720,
        960, and sharp. The world is drawn at that many pixels across and put on your
        screen at whatever size your screen is, with no filtering in between — so the
        sea's glitter is a dance of individual lit pixels rather than a sheen, and the
        rigging is a stepped line. None of it is a filter over a modern image; it is a
        genuinely small image, which is why it does not look like one with an effect on
        it.</p>

        <p>720 is what the game does unless you tell it otherwise, and it is where
        everybody who has sat down with this has ended up. 960 is the same picture
        finer, and it is worth knowing about if you are on a large display, where the
        coarse one stops reading as a drawn picture and starts reading as a
        low-resolution one. <em>sharp</em> turns the grain off entirely, for when you
        want to see what is actually on the deck.</p>

        <p>The console and the chart stay crisp at every setting. They are drawn
        afterwards, in screen space: a pixelated instrument is a nostalgic idea rather
        than a readable one.</p>""",
    ),
    (
        "the-boat",
        "The boat",
        """<p>A production charter sloop of about 34 feet: 9.20&nbsp;m on the waterline,
        5,500&nbsp;kg displacement, 64&nbsp;m² of sail, hull speed 7.36 knots. She carries
        a mainsail with three slab reefs and a wardrobe of headsails — genoa, code&nbsp;0,
        gennaker, spinnaker — one of which is set at a time.</p>

        <p><b>She is not fast and she is not meant to be.</b> Best upwind VMG in twelve
        knots of true wind lands at 40 degrees off the wind: 5.6 knots through the water,
        4.1 of velocity made good, 21 degrees of heel, 3.6 degrees of leeway. Those are the
        numbers off a real boat, and the polar on the front page is generated rather than
        drawn.</p>""",
    ),
    (
        "your-first-passage",
        "Your first passage",
        """<p>The mission is the oldest job on the coast: you are a delivery skipper, and
        somebody's boat has to be somewhere by a certain time. The panel in the top left
        says where you are bound, how far it is, what the bearing is and how long you have.</p>

        <ol>
        <li><b>Look at the glass before you look at anything else.</b> OUTLOOK on the
        console tells you what the barometer is doing, which is what decides whether you
        are about to be overpowered. Rising and steady is a pleasant afternoon; falling and
        backing is a reef you should put in now rather than in twenty minutes.</li>

        <li><b>Set a headsail and sheet the main.</b> <code>X</code> and <code>Z</code> walk
        the wardrobe — and what they do is <em>order</em> a sail rather than switch one on.
        The foredeck rolls the old one away, bends the new one on and sets it, which is a
        job with stages and takes the time it takes. <code>W</code> and <code>S</code> are
        the mainsheet; <code>Q</code> and <code>E</code> are the genoa winch.</li>

        <li><b>Steer with the tiller and watch the telltales.</b> The tiller holds the angle
        you leave it at — it is not a spring. <code>Space</code> centres it.</li>

        <li><b>Hand the sails to somebody else when you want to navigate.</b>
        <code>Shift&nbsp;T</code> calls the watch on deck: they trim the sheets continuously
        to the wind each sail actually meets, work the shape controls one line at a time,
        and choose the right sail for the angle and the breeze. A hand on any sheet stands
        them down, the way a hand on the tiller takes the helm from the pilot.</li>

        <li><b>Put the pilot on for a long leg.</b> <code>P</code> engages it and
        <code>O</code> chooses which of the two it is — a tiller pilot holding a compass
        course, or a vane holding an angle to the wind.</li>

        <li><b>Fix your position before you need it.</b> Not when you are closing the land
        in failing light. Three bearings, crossed, entered in the log.</li>
        </ol>""",
    ),
    (
        "the-instruments",
        "The instruments",
        """<p>The case is one glass in the corner, in every view. The top half is the
        compass card as you see it looking down into the bowl, with a window at the lubber
        line. The bottom half is the sea, cut as a segment of the same glass, so that
        however far she lies over, the water stays inside the bowl. The boss in the middle
        is the wind she is actually in: apparent on the needle, true behind it.</p>

        <p><b>The bezel is the part that is navigation.</b> On it: where the wind is from,
        the shaded water she cannot sail into, and the two headings that bound it — which is
        what a beat consists of. <em>Can I lay it on this tack</em> is read straight off the
        ring instead of being worked out from three numbers, and the angle comes from her
        own polar rather than from a book, so it narrows when she is reefed and opens when
        she is footing.</p>

        <p><b>Nothing is printed beside the drawing of itself.</b> Her head is the card, the
        apparent wind is the needle, her heel is the line across the bottom of the bowl — so
        those three figures are not <em>also</em> set in type next to them, which is the
        oldest way to make an instrument panel unreadable. What is printed is everything the
        glass cannot show: the log, the strength of both winds, what she is making good, the
        water under her, and what the barometer is doing.</p>

        <p><b>Each view gets the tools its job needs.</b> At the helm you are working the
        sheets, the preventer and the pilot, so the rack of seven control lines is pinned in
        the middle. From astern you are watching her sail and thinking about what is set.
        Under the water you are fishing, and there is nothing on this boat you could adjust
        from down there.</p>""",
    ),
    (
        "when-it-blows",
        "When it blows",
        """<p><b>Ease before you reef.</b> That is the order that matters: a slab is twenty
        minutes of work and a sheet is a second. The dump — <code>R</code> — eases until the
        deck is steady and the pilot can hold her; pressed again, or held, it lets
        everything go, and she stops with the cloth flogging.</p>

        <p><b>Then reef, in stages.</b> Three slabs and then she is handed: full, one, two,
        three, down. Each slab takes a sixth of her off, and the step from the second reef to
        no mainsail at all is the one you need in the forties. The reef and the furling line
        are lines in the clutch bank — <code>1</code> and <code>2</code> pick one,
        <code>A</code> and <code>D</code> work it.</p>

        <p><b>Rig the preventer for a long run and take it off before you gybe.</b>
        <code>F</code>. The whole point of it is that it stops the boom coming across, which
        is also exactly what makes it dangerous if you forget it is there.</p>

        <div class="caveat"><b>The squall is the thing to watch</b><p>It arrives on the
        console before it arrives on the boat: bearing, distance, and whether it is passing
        or coming for you. A squall that has been announced and ignored is the commonest way
        to be knocked down in this game, because the wind in it both builds and veers.</p></div>""",
    ),
    (
        "man-overboard",
        "If somebody goes in",
        """<p>There is a horseshoe lashed to the starboard quarter on a short strop, twenty
        metres of floating line coiled on the same cleat, and a canvas grab bag over them.
        <code>Enter</code> throws the ring.</p>

        <p><b>The throw is the part that cannot be late.</b> A person in the water makes two
        per cent of the wind speed and a ring of foam makes four to eight, so the ring walks
        away downwind at fifteen metres a minute. Thrown early it is something he can swim
        to; thrown after a minute of getting organised it is upwind of nothing. Those are
        search-and-rescue numbers rather than invented ones, and the whole drill turns on
        them.</p>

        <p><b>Then get her back to him and pick him up.</b> The boathook reaches by itself —
        you do not aim it — and <code>Enter</code> takes the catch. <em>When</em> is the
        whole of it: a hook put on while the deck is dropping away is a hook torn out of
        your hands, and the sea decides when that is, not the player.</p>

        <p><code>Esc</code>, then the <b>MISSIONS</b> page, practises the lot on a dan
        buoy — and it is also the best way into the game, because it teaches you to sail
        her on the way.</p>

        <p><b>The pattern it coaches is the figure of eight</b>: bear away onto a beam
        reach and sail four boat lengths, tack, work down to leeward of him, then come up
        at him on a close reach with everything eased, and arrive stopped. That is what the
        RYA and US Sailing both teach for a yacht with the casualty in sight, and it works
        under sail because it never asks her to gybe and ends on a close reach — the only
        point of sail you can stop a boat on. The Williamson turn out of the IAMSAR manual
        is the right answer for a ship under power and the wrong one here.</p>

        <p>Each leg comes with a target heading, an arrow, and a mark put in the water at
        the point she should be steering for. That hand-holding is deliberate: a beginner
        told to <em>work down to leeward of him</em> has no idea what that means, and a
        beginner steering at a lit mark learns what it means by doing it. The coaching
        inside each leg is advice rather than a gate — the drill ends when Oscar is aboard,
        however you managed it.</p>""",
    ),
    (
        "the-pilot-job",
        "Taking a ship in",
        """<p>The oldest job on this coast, and the one that pays. A square-rigged brig
        comes in from the open sea wanting somebody who knows the water, and you are the
        boat that knows it.</p>

        <p><b>Pilotage is leading, not boarding.</b> Nobody steps across — you take the
        flag, you catch her up, and you get out in front of her. From there she follows
        your water, which means every rock you clear she clears and every one you cut fine
        she cuts finer, in a hull that draws more than yours and cannot turn like
        yours.</p>

        <p>The flag <b>H</b> — <em>I have a pilot aboard</em> — goes up in your own rigging
        the moment you take the job, and stays there until the work is done. The chart
        draws the water you ought to be leading her through.</p>

        <p><b>It ends on the water, not alongside.</b> Bring her to clear water off the
        anchorage, and <code>Enter</code> hands her back: the flag comes down and she is
        the harbour's problem from there. Which is what a pilot actually does — the berth
        is somebody else's job.</p>

        <p>The job is on the <b>MISSIONS</b> page under <code>Esc</code>, beside the
        overboard drill.</p>""",
    ),
    (
        "navigation",
        "Finding out where you are",
        """<p><b>One instrument at a time, and you get one hand.</b>
        <code>Shift&nbsp;B</code> is a pair of glasses and <code>Shift&nbsp;N</code> a hand
        bearing compass; picking one up puts the other down, because seven-by-fifties are
        two-handed on a moving boat and a compass is held out at arm's length. Either of
        them puts you in the cockpit. Spreading the chart puts both away: you work on the
        paper or you work on the horizon.</p>

        <p><b>Name the marks first.</b> <code>B</code> asks whether what is in the glasses is
        enough: a mark near enough to read its symbol names itself, and three well spread
        apart are a shape that belongs in one place on the coast — worked out the way a
        station pointer does it, from the two horizontal angles between them.</p>

        <p><b>Then shoot bearings.</b> <code>N</code>, with the notch held on the mark. It
        answers either way: the name and the number if it caught, and how far off you were if
        it did not. Holding a notch a degree and a quarter wide on a mark from a moving deck
        is the small act of steadiness the instrument exists to need.</p>

        <p><b>The book is where a navigator writes things down — not the chart.</b> Every
        bearing, every mark made out, every reading off the log goes on the page beside the
        paper with the time against it. Two buttons, because marking the paper and writing
        the book are two acts: ON THE CHART puts the position on the paper and is dead until
        two lines cross; IN THE LOG writes the line and is live always.</p>

        <p><b>Use the rubber.</b> One press takes a doubtful line off the paper and another
        puts it back where it now belongs, because the usual reason a fix will not close is a
        line you should not have believed.</p>

        <p><b>WHY</b>, on the end of the row of instruments, makes every line on the paper
        say what it is and why it is there — position line, transferred, ruled by hand, log
        leg, reckoning, track made good. A plot is six kinds of line in two colours with no
        legend, and telling them apart is most of the subject.</p>

        <div class="caveat"><b>The paper does not know she tacked</b><p>The reckoning is a
        sequence of courses, not a line, and the plot is as old as your last visit to the
        chart table. She knows she tacked — the compass is on the bulkhead — but the paper
        goes on laying off the course it was last told, with the miles off the log, and the
        pencil position walks away from her. Ten minutes of unwritten log after a
        ninety-degree tack is a mile and a half of pencil error, measured. Spreading the
        chart writes it up.</p></div>""",
    ),
    (
        "racing",
        "The start",
        """<p><b>The one thing sailing has that nothing else does is the start.</b> You
        cannot stop a sailing boat and you cannot park her — she is always going
        somewhere — so the start of a race is not a button but a manoeuvre. You spend the
        last five minutes making the boat arrive at a line she must not touch early, at
        full speed, at the end of it you have decided is favoured. Everybody who has
        raced knows the race is usually decided there, and everybody who has not is
        astonished by it.</p>

        <p><b>The favoured end is calculated, not invented.</b> A line a hundred and
        forty metres long that is ten degrees off square to the wind hands you twenty-odd
        metres of free distance at one end of it, and the panel says which end and by how
        many metres — metres being the only unit you can think in on the water. Over
        early and you come back, which is the one rule that makes a start a start.</p>

        <p><b>Four legs, windward and leeward, about twenty minutes.</b> The course is
        laid off the wind at the moment it is set: one buoy dead upwind, another dead
        downwind, so every leg is a beat or a run and nothing in between. That is
        deliberate — those are the two points of sail with something to decide on them.
        A reach is a lovely thing to sail and there is nothing to do on it but steer, and
        a course made of reaches is a course where the boat that started first wins.</p>

        <p>The marks are rounded by being near them — twenty-five metres — because which
        side you passed is a question this game cannot answer honestly, and a rule it
        cannot enforce is better left to the sailor.</p>

        <div class="caveat"><b>What is not in it yet</b><p>No other boats. The fleet
        exists and the times that would make it a race are two fields on the wire, but
        there is nobody to start against, and so no rules of the road either — boats here
        do not collide, and a protest nobody can lodge is a rule nobody has. What is here
        is the clock, the line, the marks, and being sent back when you are over
        early.</p></div>""",
    ),
    (
        "fishing",
        "The rod",
        """<p>She was an open fishing boat until 1964, so there is a rod in a holder on her
        port quarter. It is not a minigame beside the sailing; it is bolted to it, and the
        join is one fact: <b>a fish on the line at six knots is a fish you are going to
        lose.</b> The water dragging a nine-kilo ling broadside at that speed pulls harder
        than the line will stand, so the first thing you do when something takes is get the
        way off her — and the gear for that is the mainsheet, the dump, the backed headsail
        and the helm you already have. The rod has no speed control. The boat is the speed
        control.</p>

        <p><b>Depth is the boat's speed.</b> At five knots a trolled lure skips along in the
        first metre or two; take the way off her and it sinks hard — nine metres at two
        knots and thirty at one. That is why mackerel are caught fast and shallow and cod are
        caught slow and deep: the boat chooses the quarry before the tackle does. The splash
        astern is the depth gauge, and it is gone by four metres.</p>

        <p><b>The drag is the whole game, and it is wrong in both directions.</b> Too tight
        and the hook comes out of the fish's mouth, because load is what opens the hole it
        sits in. Too light and the fish never tires at all — fatigue is charged for work done
        against the line, so a drag you have backed off is a fish you are not beating. The
        rule printed on every spool is a quarter to a third of the line's breaking strain,
        and that band is painted on the dial.</p>

        <p><b>Lean on it and it goes.</b> A fish with something left does not hang there and
        take heavy pressure; it runs. Pull harder and you get a longer fight, which is the
        first thing anybody is told on a boat.</p>

        <p><b>Nothing comes quietly, and the last move is timed.</b> Every fish, however
        beaten, finds one more run at the sight of the hull. What is left after that is not
        strength but timing: a fish alongside is lifted <em>on the rise</em>, letting the sea
        bring it up to the rail. One swung aboard as she drops away takes its whole weight on
        a hook that has been working loose for five minutes.</p>""",
    ),
    (
        "sailing-together",
        "Sailing together",
        """<p>Two to eight friends in one archipelago. Somebody runs the relay:</p>

        <pre>sail-server --seed 42</pre>

        <p>Everybody else presses <code>Esc</code> and opens the <b>TOGETHER</b> page. A game
        on the same network is in the list. The world comes from whoever is hosting — join
        and you are in their archipelago, their weather and their time of day. Everybody's
        boat is on the chart with a name beside it.</p>

        <p>A game on a server rather than a laptop has an address and a <b>word</b>, and both
        go in that page. <b>KNOCK</b> asks what is there without joining it — the game, how
        many are aboard, which sea — and when nothing answers it says which of the three
        things it is: the word, the port, or a server that is not running. All three used to
        look identical and all three are different evenings.</p>

        <p>Over the internet the same thing works with the host's address typed in, and the
        least-work answer is one Tailscale network between friends: no port forwarding and no
        code.</p>""",
    ),
    (
        "when-something-is-wrong",
        "When something is wrong",
        """<ul>
        <li><b>She will not point.</b> Check what is set and what is in. A reefed boat
        cannot lay what a full-rigged one can, and the shaded sector on the compass bezel
        narrows to tell you so.</li>

        <li><b>She is in irons.</b> Back the genoa — <code>G</code>. It is the only sail that
        can be backed and it is the way out.</li>

        <li><b>The pilot let go.</b> The electric one says so: a controller that has been on
        the stops and off the course together for six seconds knows it is braking rather than
        steering. The vane has nothing to let go with — overpowered it does not disengage, it
        stops coping, and in the end she tacks or gybes herself and the gear goes on patiently
        steering the new course.</li>

        <li><b>The sails are flogging and nothing you do helps.</b> You are over-canvassed.
        Ease, then reef.</li>

        <li><b>The fix will not close.</b> One of the three lines is wrong — most likely a
        misidentified mark. Rub it out and take another.</li>

        <li><b>Something looks wrong and you cannot describe it.</b> <code>F12</code> puts a
        picture of the screen in your Pictures folder. The fault that cannot be described is
        the one nobody fixes.</li>
        </ul>""",
    ),
]


# ---- the pictures, in the order they tell the story ------------------------

SHOTS = [
        (
            "sunset.webp",
            "The end of the day",
            "Dawn and dusk used to be the same grey as noon: only the sunlight warmed, "
            "so the water carried a gold path under a sky with no colour in it. The "
            "gradient moves with the sun now — deep towards violet overhead, burning "
            "where the sun is, with the anti-twilight arch on the far side — and the "
            "cloud bases catch what the sea throws back up at them.",
        ),
        (
            "brig.webp",
            "A brig wants a pilot",
            "She comes in from the open sea and will not come to you. Pilotage is leading, "
            "not boarding: catch her up, get out in front, and she follows your water from "
            "there — in a hull that draws more than yours and cannot turn like yours.",
        ),
        (
            "chart.webp",
            "The chart table",
            "Dividers, a plain ruler, a parallel rule, a protractor, a pencil, a "
            "stopwatch, a hand log and a rubber — and the deck log beside the paper, "
            "because a chart is not where a navigator writes things down. The coast is "
            "read off the same field the boat runs aground on, so it cannot lie about "
            "the depths.",
        ),
        (
            "lost.webp",
            "Somewhere at sea, with no idea where",
            "The names come off the marks and the chart stops following her; getting them "
            "back is the exercise. Seeing a thing and knowing which thing it is are "
            "different questions, and the second is the whole difficulty — three marks "
            "well spread apart are a shape that belongs in one place on the coast and "
            "nowhere else.",
        ),
        (
            "squall.webp",
            "A squall, announced",
            "Bearing, distance, and whether it is passing or coming for you — on the "
            "console before it is on the boat. The wind in it both builds and veers, and a "
            "squall that has been announced and ignored is the commonest way to be knocked "
            "down here.",
        ),
        (
            "gennaker.webp",
            "The wardrobe",
            "Genoa, code 0, gennaker, spinnaker. Asking for one is an order rather than a "
            "switch: the foredeck rolls the old sail away, bends the new one on and sets "
            "it, which is a job with stages and takes the time it takes.",
        ),
        (
            "race.webp",
            "Fifteen seconds to the gun",
            "You cannot stop a sailing boat and you cannot park her, so the start of a "
            "race is not a button but a manoeuvre: five minutes to sail away from the "
            "line, read the wind, pick the favoured end and arrive at full speed without "
            "being early. The panel here is saying you will not make it — 233 m to the "
            "line and 124 seconds of sailing to cover it — and that the line is square, "
            "so neither end is worth the detour.",
        ),
        (
            "drill.webp",
            "Taught, not announced",
            "The overboard drill coaches the figure of eight — the reach, tack, reach the "
            "RYA and US Sailing both teach — one instruction at a time, with a target "
            "heading and a mark in the water for each leg. When something has to be done "
            "in the next two seconds it is four words across the screen.",
        ),
        (
            "below.webp",
            "Under the surface, beside the lure",
            "This project argued against the view on the grounds that the sea is opaque "
            "and there would be nothing to see — which was an argument about the "
            "surface. Ten metres of visibility, green rather than blue because this is a "
            "shelf sea with a spring bloom in it.",
        ),
        (
            "rod.webp",
            "A rod over the stern",
            "Not a minigame beside the sailing. A fish on the line at six knots is a fish "
            "you are going to lose, so the first thing you do when something takes is get "
            "the way off her — with the mainsheet, the dump and the helm you already have.",
        ),
]

# Captions for the three pictures that are not part of the gallery sequence.
REEL_CAPTION = (
    "Thirty frames of a squall",
    "She heels, the sea runs under her, and the instruments answer — which is the half "
    "of this that a still photograph cannot say. Recorded by the game photographing "
    "itself, because screen capture wants a permission a build server does not have.",
)

HELP_SHOT_CAPTION = (
    "The same list, in the program",
    "Behind a mark in the corner, which is the first thing anybody tries. The page "
    "above is generated from the source of this screen.",
)

WIDTHS_CAPTION = (
    "720 on the left, 960 on the right",
    "The same shrouds, the same moment, enlarged equally. On open water the two are "
    "indistinguishable; it is rigging that shows the difference, which is also what the "
    "coarsest width used to lose.",
)

REEL_ALT = "The boat sailing in a squall, heeled over"
HELP_SHOT_ALT = "The help screen in the game"
WIDTHS_ALT = "The same rigging drawn at 720 pixels across and at 960"


# ---- the install section, which is prose round two commands ---------------

INSTALL_MAC = """A line in a terminal rather than a download on purpose: macOS
quarantines what a <em>browser</em> saves and not what <code>curl</code> fetches, and
since macOS 15 removed right-click-Open there is otherwise no straightforward way past
<em>&ldquo;Sail is damaged and can't be opened&rdquo;</em> on an application nobody has
paid Apple to vouch for."""

INSTALL_WINDOWS = """Same idea: <em>&ldquo;Windows protected your PC&rdquo;</em> comes
from the mark of the web, which the program doing the downloading attaches — a browser
does, <code>Invoke-WebRequest</code> does not. <code>Sail.exe</code> from the releases
page works too, if you would rather click <b>More info</b> and <b>Run anyway</b>."""

SOURCE_GAME = "the game"
SOURCE_TESTS = "the validation suite"
SOURCE_POLAR = "the speed polar"
