"""Polska wersja tekstów ze `content.py`.

Osobny plik, a nie słownik tłumaczeń wewnątrz tamtego, z dwóch powodów. Teksty są
długie i pisane prozą — para `oryginał: przekład` w jednej linii jest nieczytelna przy
akapicie na osiem zdań. A przekład nie jest tu kalką: angielski tekst pozwala sobie na
rytm, którego polszczyzna nie znosi, więc zdania bywają przestawione, a nie przepisane.

Czego tu nie ma i dlaczego — opisane w `i18n.py`: lista klawiszy, polara i tabela
modelu zostają po angielsku, bo są generowane z gry i z README.
"""

TAGLINE = "Symulator żeglowania w duchu symulatora ciężarówki dalekobieżnej."

STANDFIRST = (
    "Prowadzisz niewielki jacht turystyczny, a rzecz jest w przelocie, nie w wyścigu. "
    "<strong>Wiatr pozorny, trym żagli, przechył i moment prostujący, dryf, ściana "
    "prędkości kadłubowej, przeciągnięcie steru, nawietrzność</strong> — policzone "
    "naprawdę, a nie strzałka wiatru i mnożnik prędkości."
)

WHAT_IT_IS = [
    (
        "Pętlą jest samo żeglowanie",
        """Wiatr rzeczywisty wyznacza trym; trym daje napęd i dryf; napęd przesuwa wiatr
        pozorny; i trymujesz od nowa. Wszystko inne — kotwiczenie, silnik pomocniczy,
        kabestany, refowanie — jest przedłużeniem tej pętli, a nie czymś obok niej.""",
    ),
    (
        "Kąt natarcia, nie kąt wiatru",
        """Program liczący prędkość może założyć idealny trym i ująć wszystko w tabelę
        według samego kąta wiatru. Gra nie może, bo trymowanie jest robotą gracza. Szot
        ustawia kąt cięciwy, cięciwa ustawia kąt natarcia, a kąt natarcia decyduje, czy
        żagiel ciągnie, łopocze, czy jest przeciągnięty.""",
    ),
    (
        "Uczy żeglować",
        """Ćwiczenie „człowiek za burtą” jest zarazem lekcją: jedno polecenie naraz, nic
        dalej, dopóki poprzednie nie zostanie wykonane — a potem boja idzie za burtę i
        cała reszta zaczyna mieć znaczenie. Rozpędzenie łodzi to trzy klawisze i dwie
        minuty; sprowadzenie jej z powrotem w jedno miejsce i <em>zatrzymanie</em> tam to
        całe prowadzenie jachtu, i nikt tego nie ćwiczy, dopóki nie musi.""",
    ),
    (
        "Jedna łódź, zrobiona porządnie",
        """Seryjny slup czarterowy, około trzydziestu czterech stóp — klasa, którą
        najwięcej żeglarzy naprawdę prowadziło i potrafi ocenić. Jedna łódź zrobiona
        porządnie bije pięć zrobionych pobieżnie.""",
    ),
]

MODEL_INTRO = """Każdy wiersz to arytmetyka, która może się mylić w sposób zauważalny dla
żeglarza — i dlatego rdzeń niesie zestaw testów sprawdzających go względem liczb znanych
z wody, a nie względem jego własnych wyników."""

NAVIGATION = """Archipelag ma nazwy, a ustalenie, na który jego kawałek się patrzy, jest
grą samą w sobie — najstarszą, jaka istnieje, i tą, którą skasował ploter nawigacyjny."""

NAVIGATION_POINTS = [
    (
        "Mapa jest z papieru",
        """<code>M</code> kładzie ją w rogu; <code>Shift&nbsp;M</code> rozkłada na stole
        wraz z przyrządami, które ma stół nawigacyjny — cyrklem, linijką równoległą,
        kątomierzem, ołówkiem, stoperem, logiem ręcznym i gumką. Brzeg na niej jest
        odczytany z tego samego pola odległości, na którym łódź wchodzi na mieliznę, więc
        mapa nie może skłamać o głębokościach: jest tylko jeden ich komplet.""",
    ),
    (
        "Zobaczyć rzecz i wiedzieć, która to rzecz",
        """To dwa różne pytania, a drugie jest całą trudnością. Latarnia jest pomalowana
        tak, żeby dało się ją rozpoznać, i świeci; stawa musi być na tyle blisko, żeby dało
        się odczytać jej znak szczytowy — inaczej jest patykiem. Lornetka mniej więcej
        podwaja odległość, z jakiej znak da się <em>nazwać</em>, a trzy dobrze rozstawione
        znaki to kształt, który pasuje do jednego miejsca na wybrzeżu i do żadnego
        innego.""",
    ),
    (
        "Namiar to linia, nie pozycja",
        """Kompas ręczny ma szczerbinkę szeroką na jakiś stopień z ćwiercią, a liczba
        niesie błąd, który róża w kotwiczce naprawdę ma — dwa albo trzy stopnie na fali.
        Dlatego pozycja jest trójkątem, a nie punktem, dlatego ostre przecięcie to zła
        pozycja i dlatego trzecia linia jest warta wzięcia.""",
    ),
    (
        "Pozycja z przeniesienia",
        """Jeden znak, dwa namiary i droga między nimi dają pozycję. Każda linia pozycyjna
        jest kreślona przez znak przesunięty o drogę przebytą od chwili wzięcia namiaru —
        świeża przechodzi przez znak, a dwadzieścia minut później leży o szerokość dłoni
        obok niego, przerywana. Najstarszy chwyt na wybrzeżu z jedną latarnią.""",
    ),
    (
        "Zgubiony na własne życzenie",
        """<code>Shift&nbsp;L</code> stawia cię gdzieś na morzu bez pojęcia gdzie. Mapa
        przestaje za tobą chodzić, nazwy znikają ze znaków, a odzyskanie ich jest
        ćwiczeniem — ocenianym uczciwie: pozycja jest dobra, jeśli leży blisko miejsca,
        w którym łódź naprawdę jest, a nie jeśli trójkąt wyszedł mały. Ciasny trójkąt na
        dwóch źle rozpoznanych znakach to pewne siebie kłamstwo i klasyczny sposób, żeby
        wsadzić łódź na skałę.""",
    ),
]

SOUND = """Syntezowany, nie próbkowany. W repozytorium nie ma plików dźwiękowych i nie
muszą być: wiatr i woda <em>są</em> filtrowanym szumem, więc generowanie ich jest metodą
uczciwą, a nie drogą na skróty. Dźwięk olinowania bierze się z zależności Strouhala na
wancie sześciomilimetrowej, a bramkuje go to, jak <em>równy</em> jest wiatr — wiry nie
potrafią się zsynchronizować z przepływem, który się pod nimi zmienia, więc wanta w
porywie puszcza, milknie i znajduje nową nutę, kiedy poryw się ustali."""

SOUND_BAND = """Jest też kapela, i to łódź ją pisze, a nie ktoś gra przy niej. Tempo bierze
się z okresu spotkań — częstości, z jaką kadłub naprawdę spotyka fale — więc muzyka idzie
w rytmie żeglowania, a nie w poprzek niego. Kurs względem wiatru wybiera metrum i język:
pod wiatr jest synkopowane i kanciaste, półwiatr dostaje chodzący bas, baksztag jest
rzadki. Poza tym, co łódź może znieść, język zmienia się całkiem — na coś ze zmniejszoną
sekundą."""

TOGETHER = """Od dwóch do ośmiu znajomych w jednym archipelagu. Każdą łódź liczy ten, kto
nią żegluje, a serwer jest przekaźnikiem, zegarem i poczekalnią — cokolwiek innego
wstawiłoby czas obiegu między czyjąś rękę a ster, a żeglarz czuje pięćdziesiąt milisekund
na rumplu. Gra w tej samej sieci jest na liście, bo adres podyktowany przez czat jest
błędny mniej więcej za trzecim razem."""

NOT_IN_THE_BOX = """<b>Jeszcze nie policzone i wiadomo, że tego brak: pływy i prądy.</b>
Mapa rysuje w rogu znoszenie wiatrowe, dwa i pół procenta siły wiatru, i tekstura morza
jest przez nie niesiona — ale kadłub już nie, więc ta strzałka jest jedyną rzeczą na
papierze, której łódź nie słucha. Warto o tym wiedzieć, zanim się zaufa pozycji
zliczonej. Czas płynie jeden do jednego, bez kompresji, co jest decyzją świadomą i taką,
do której trzeba będzie wrócić: czterdzieści mil przy pięciu węzłach to osiem godzin."""

PICTURES_AND_A_TAB = """<b>To jest wersja z biurka, a w karcie można pożeglować od
razu.</b> Całość kompiluje się na <code>wasm32-unknown-unknown</code> i przycisk na górze
strony to otwiera: to samo morze, ta sama łódka, ten sam model, jakichś piętnaście
megabajtów po drucie i niecałe pięćdziesiąt klatek na sekundę. Czego karta nie potrafi,
to wspólnego żeglowania — przeglądarka nie ma UDP i nie umie rozgłaszać, więc przekaźnik,
lista gier w sieci i dołączanie po adresie zostają przy wersji z biurka — i potrzebuje
prawdziwej klawiatury, więc telefon musi poczekać."""

WHY_NOT_BROWSER = """<b>Nie ma tu dema do zagrania w karcie przeglądarki.</b> Całość
<em>kompiluje się</em> na <code>wasm32-unknown-unknown</code>, więc nie to jest przeszkodą
— przeszkodą jest wspólne żeglowanie. Przeglądarka nie ma UDP i nie umie rozgłaszać, więc
znika przekaźnik, lista gier w sieci lokalnej i dołączanie po adresie, a w zamian pojawia
się certyfikat i publiczny host. Zdjęcia na tej stronie pokazują, jak program wygląda; sam
program jest dwie linijki niżej."""

MANUAL_STANDFIRST = """Wszystko, czego trzeba, żeby odejść od boi, przejść przelot i wejść
do następnego portu — oraz co robić, kiedy coś idzie nie tak. Lista klawiszy jest
generowana z ekranu pomocy w samej grze, czyli z jedynej kopii, która nie może się
rozjechać."""

MANUAL = [
    (
        "before-you-start",
        "Zanim zaczniesz",
        """<p>Nic tutaj nie wymaga konta, launchera ani sieci. Na Macu albo na pececie to
        jedna linijka w terminalu; ze źródeł jedno polecenie <code>cargo</code>. Linie
        instalacyjne są na stronie głównej i w pliku README repozytorium.</p>

        <p><b>Gra jest pisana pod gamepada; klawiatura też działa.</b> Reguła jest taka:
        przypisuje się <em>zamiary</em>, a nie liny, i dobiera naturę każdego
        sterowania do sprzętu, który ono zastępuje — ster i szot grota to pozycje
        trzymane ręką, szot genui to kabestan, który się kręci, a ref to robota do
        wykonania.</p>

        <p><b>Kamera w kokpicie przechyla się razem z łodzią.</b> To jest celowe. Przechył
        jest pierwszą rzeczą, po której poznaje się, co dzieje się z łodzią, a widok
        trzymający poziom tę informację wyrzuca.</p>

        <p><b>Obraz ma ziarno i możesz wybrać, jak grube.</b> <code>Esc</code>, potem
        <b>PICTURE</b>: wiersz <em>pixels</em> daje 720, 960 i <em>sharp</em>. Świat jest
        rysowany w takiej liczbie pikseli i rozciągany na ekran bez żadnego filtrowania po
        drodze — dlatego połysk morza jest tańcem pojedynczych rozświetlonych pikseli, a
        nie połyskiem, a olinowanie jest linią schodkową. Nic z tego nie jest filtrem
        nałożonym na współczesny obraz; to jest naprawdę mały obraz i dlatego nie wygląda
        jak duży z efektem.</p>

        <p>720 jest tym, co gra robi, jeśli jej nie powiesz inaczej, i tam lądują
        wszyscy, którzy przy tym siedzieli. 960 to ten sam obraz, drobniejszy — warto o
        nim wiedzieć przy dużym ekranie, gdzie grubsze ziarno przestaje czytać się jako
        obraz rysowany, a zaczyna jako niskorozdzielczy. <em>sharp</em> wyłącza ziarno
        całkiem, na chwile, kiedy chcesz zobaczyć, co naprawdę leży na pokładzie.</p>

        <p>Konsola i mapa zostają ostre przy każdym ustawieniu. Są rysowane później, w
        przestrzeni ekranu: spikselizowany przyrząd to pomysł nostalgiczny, a nie
        czytelny.</p>""",
    ),
    (
        "the-boat",
        "Łódź",
        """<p>Seryjny slup czarterowy około trzydziestu czterech stóp: 9,20 m linii wodnej,
        5500 kg wyporności, 64 m² ożaglowania, prędkość kadłubowa 7,36 węzła. Ma grota z
        trzema refami i garderobę przednich żagli — genuę, code&nbsp;0, gennakera,
        spinakera — stawia się jeden naraz.</p>

        <p><b>Nie jest szybka i nie ma być.</b> Najlepsze VMG pod wiatr przy dwunastu
        węzłach wypada na 40 stopniach od wiatru: 5,6 węzła przez wodę, 4,1 zyskane, 21
        stopni przechyłu, 3,6 stopnia dryfu. To są liczby z prawdziwej łodzi, a polara na
        stronie głównej jest wygenerowana, nie narysowana.</p>""",
    ),
    (
        "your-first-passage",
        "Pierwszy przelot",
        """<p>Zlecenie jest najstarszą robotą na wybrzeżu: jesteś skipperem przeprowadzeń,
        a czyjaś łódź ma być gdzieś na czas. Panel w lewym górnym rogu mówi, dokąd idziesz,
        jak daleko to jest, jaki jest namiar i ile masz czasu.</p>

        <ol>
        <li><b>Najpierw spójrz na barometr, zanim spojrzysz na cokolwiek innego.</b>
        OUTLOOK na konsoli mówi, co robi ciśnienie, a to decyduje, czy zaraz będziesz
        przeciążony. Rosnące i stałe to miłe popołudnie; spadające i cofające to ref, który
        trzeba założyć teraz, a nie za dwadzieścia minut.</li>

        <li><b>Postaw przedni żagiel i wybierz grota.</b> <code>X</code> i <code>Z</code>
        przechodzą przez garderobę — i jest to <em>rozkaz</em>, a nie przełącznik. Dziób
        zwija stary żagiel, podpina nowy i stawia go, co jest robotą z etapami i trwa tyle,
        ile trwa. <code>W</code> i <code>S</code> to szot grota; <code>Q</code> i
        <code>E</code> to kabestan genui.</li>

        <li><b>Steruj rumplem i patrz na nitki.</b> Rumpel trzyma kąt, w jakim go
        zostawisz — to nie jest sprężyna. <code>Space</code> ustawia go na wprost.</li>

        <li><b>Oddaj żagle komuś, kiedy chcesz nawigować.</b> <code>Shift&nbsp;T</code>
        woła wachtę na pokład: trymują szoty na bieżąco, do wiatru, który każdy żagiel
        naprawdę spotyka, pracują przy jednej linie naraz i dobierają żagiel do kąta i siły
        wiatru. Ręka na dowolnym szocie odsyła ich, tak samo jak ręka na rumplu odbiera
        ster autopilotowi.</li>

        <li><b>Na długi odcinek włącz autopilota.</b> <code>P</code> go załącza, a
        <code>O</code> wybiera, który to z dwóch — autopilot sterujący kursem kompasowym
        albo wiatrowy trzymający kąt do wiatru.</li>

        <li><b>Ustal pozycję, zanim będzie potrzebna.</b> Nie wtedy, kiedy podchodzisz do
        lądu o zmierzchu. Trzy namiary, przecięte, zapisane w dzienniku.</li>
        </ol>""",
    ),
    (
        "the-instruments",
        "Przyrządy",
        """<p>Cała skrzynka to jedna kotwiczka w rogu, w każdym widoku. Górna połowa to róża
        kompasowa widziana tak, jak się ją widzi patrząc w kotwiczkę, z okienkiem przy
        kresce kursowej. Dolna to morze, wycięte jako odcinek tego samego szkła, żeby
        niezależnie od przechyłu woda została w kotwiczce. Guzik pośrodku to wiatr, w
        którym łódź naprawdę jest: pozorny na wskazówce, rzeczywisty za nim.</p>

        <p><b>A pierścień jest tą częścią, która jest nawigacją.</b> Na nim: skąd wieje,
        zacieniona woda, w którą nie da się wejść, i dwa kursy, które ją ograniczają — czyli
        to, z czego składa się halsowanie. <em>Czy dojdę tam tym halsem</em> odczytuje się
        wprost z pierścienia, zamiast wyliczać z trzech liczb, a kąt bierze się z jej
        własnej polary, nie z książki, więc zwęża się przy refie i otwiera, kiedy łódź
        odpada.</p>

        <p><b>Nic nie jest napisane obok swojego rysunku.</b> Kurs łodzi to róża, wiatr
        pozorny to wskazówka, przechył to linia w dolnej połowie — więc te trzy liczby nie
        są <em>dodatkowo</em> wypisane cyframi obok, co jest najstarszym sposobem na zrobienie
        nieczytelnego pulpitu. Wypisane jest wszystko, czego kotwiczka pokazać nie może:
        log, siła obu wiatrów, prędkość zyskana, woda pod kilem i to, co robi
        barometr.</p>

        <p><b>Każdy widok dostaje narzędzia do swojej roboty.</b> Przy sterze pracujesz
        szotami, bomblokiem i autopilotem, więc listwa siedmiu lin jest przypięta
        pośrodku. Z rufy patrzysz, jak żagle pracują, i myślisz o tym, co jest postawione.
        Pod wodą łowisz i nie ma na tej łodzi niczego, co dałoby się stamtąd
        wyregulować.</p>""",
    ),
    (
        "when-it-blows",
        "Kiedy przywieje",
        """<p><b>Najpierw luzuj, dopiero potem refuj.</b> To jest kolejność, która ma
        znaczenie: ref to dwadzieścia minut roboty, a szot to sekunda. Zrzut —
        <code>R</code> — luzuje, aż pokład się uspokoi i autopilot da radę ją utrzymać;
        naciśnięty drugi raz albo przytrzymany puszcza wszystko i łódź staje z łopoczącym
        płótnem.</p>

        <p><b>Potem refuj, etapami.</b> Trzy refy i jest obezżaglowana: pełny, pierwszy,
        drugi, trzeci, dół. Każdy ref zabiera jej szóstą część, a krok od drugiego refu do
        zupełnego braku grota jest tym, którego potrzebujesz przy czterdziestu węzłach. Ref
        i lina rolera są linami w banku sprzęgieł — <code>1</code> i <code>2</code> wybierają,
        <code>A</code> i <code>D</code> pracują.</p>

        <p><b>Na długi baksztag załóż bomblok i zdejmij go przed zwrotem przez rufę.</b>
        <code>F</code>. Cały jego sens polega na tym, że nie pozwala bomowi przelecieć — co
        jest zarazem dokładnie tym, co czyni go groźnym, kiedy zapomnisz, że jest
        założony.</p>

        <div class="caveat"><b>Szkwał to rzecz do pilnowania</b><p>Pojawia się na konsoli,
        zanim pojawi się na łodzi: namiar, odległość i to, czy przechodzi bokiem, czy idzie
        na ciebie. Szkwał zapowiedziany i zlekceważony jest tutaj najczęstszym sposobem na
        położenie łodzi, bo wiatr w nim jednocześnie rośnie i skręca.</p></div>""",
    ),
    (
        "man-overboard",
        "Kiedy ktoś wypadnie za burtę",
        """<p>Na prawej ćwiartce rufy wisi koło na krótkiej stropie, dwadzieścia metrów
        pływającej liny zwiniętej na tym samym knadze i brezentowa torba ratunkowa na nich.
        <code>Enter</code> rzuca koło.</p>

        <p><b>Rzut jest tą częścią, która nie może się spóźnić.</b> Człowiek w wodzie
        dryfuje z dwoma procentami prędkości wiatru, a koło z pianki z czterema do ośmiu,
        więc koło odchodzi z wiatrem po piętnaście metrów na minutę. Rzucone od razu jest
        czymś, do czego da się dopłynąć; rzucone po minucie zbierania się jest na nawietrznej
        niczego. To są liczby z ratownictwa morskiego, a nie wymyślone, i całe ćwiczenie
        stoi na nich.</p>

        <p><b>Potem wróć do niego i podejmij go.</b> Bosak sięga sam — nie celujesz nim — a
        <code>Enter</code> bierze chwyt. Cała rzecz polega na tym, <em>kiedy</em>: bosak założony,
        gdy pokład spada, to bosak wyrwany z rąk, a o tym, kiedy to jest, decyduje morze, a
        nie gracz.</p>

        <p><code>Esc</code>, a potem strona <b>MISSIONS</b>, ćwiczy całość na boi — i jest
        zarazem najlepszym wejściem w grę, bo po drodze uczy nią żeglować.</p>

        <p><b>Schemat, którego uczy, to ósemka:</b> odpadnij na półwiatr i przejdź cztery
        długości łodzi, zrób zwrot na wiatr, zejdź pod wiatr poniżej niego, a potem podejdź
        do niego ostrym półwiatrem z wyluzowanym wszystkim i dojdź zatrzymany. Tego uczą i
        RYA, i US Sailing dla jachtu, który ma rozbitka na oku, a działa pod żaglami, bo
        nigdy nie każe przechodzić rufą i kończy się ostrym półwiatrem — jedynym kursem, na
        którym da się łódź zatrzymać. Zwrot Williamsona z podręcznika IAMSAR jest dobrą
        odpowiedzią dla statku na silniku i złą tutaj.</p>

        <p>Każdy odcinek dostaje kurs docelowy, strzałkę i znak położony na wodzie w
        miejscu, na które należy sterować. Ta pomoc jest celowa: początkujący, któremu każe
        się <em>zejść pod wiatr poniżej niego</em>, nie ma pojęcia, co to znaczy, a
        początkujący sterujący na świecący znak dowiaduje się tego, robiąc to. Podpowiedzi
        wewnątrz odcinka są radą, a nie bramką — ćwiczenie kończy się, kiedy Oscar jest na
        pokładzie, niezależnie od tego, jak to się udało.</p>""",
    ),
    (
        "the-pilot-job",
        "Wprowadzanie statku",
        """<p>Najstarsza robota na tym wybrzeżu i ta, która płaci. Bryg rejowy wchodzi z
        pełnego morza i szuka kogoś, kto zna wodę, a ty jesteś łodzią, która ją zna.</p>

        <p><b>Pilotaż to prowadzenie, nie wchodzenie na pokład.</b> Nikt się nie
        przesiada — bierzesz flagę, doganiasz go i wychodzisz przed niego. Od tej chwili
        idzie twoją wodą, co znaczy, że każdą skałę, którą ty miniesz, on też minie, a każdą,
        którą weźmiesz ciasno, on weźmie ciaśniej — w kadłubie, który ma większe zanurzenie i
        nie skręca tak jak twój.</p>

        <p>Flaga <b>H</b> — <em>mam pilota na pokładzie</em> — idzie w górę w twoim własnym
        olinowaniu w chwili przyjęcia zlecenia i wisi tam do końca roboty. Mapa rysuje wodę,
        którą powinieneś go prowadzić.</p>

        <p><b>Kończy się na wodzie, nie przy nabrzeżu.</b> Doprowadź go na czystą wodę
        przed kotwicowiskiem, a <code>Enter</code> go oddaje: flaga schodzi i od tego miejsca
        jest zmartwieniem portu. Czyli dokładnie tym, co robi prawdziwy pilot — miejsce przy
        kei jest czyjąś inną robotą.</p>

        <p>Zlecenie jest na stronie <b>MISSIONS</b> pod <code>Esc</code>, obok ćwiczenia z
        człowiekiem za burtą.</p>""",
    ),
    (
        "navigation",
        "Ustalanie pozycji",
        """<p><b>Jeden przyrząd naraz — i masz jedną rękę.</b> <code>Shift&nbsp;B</code> to
        lornetka, a <code>Shift&nbsp;N</code> kompas ręczny; wzięcie jednego odkłada drugi,
        bo siedem na pięćdziesiąt trzyma się dwiema rękami na kołyszącej się łodzi, a kompas
        wyciąga się na odległość ramienia. Każde z nich stawia cię w kokpicie. Rozłożenie
        mapy chowa oba: albo pracujesz na papierze, albo na horyzoncie.</p>

        <p><b>Najpierw nazwij znaki.</b> <code>B</code> pyta, czy to, co jest w lornetce,
        wystarczy: znak na tyle blisko, żeby odczytać jego kształt, nazywa się sam, a trzy
        dobrze rozstawione nazywają wybrzeże — liczone tak, jak robi to kątomierz nawigacyjny,
        z dwóch kątów poziomych między nimi.</p>

        <p><b>Potem bierz namiary.</b> <code>N</code>, ze szczerbinką trzymaną na znaku.
        Odpowiada w obu przypadkach: nazwą i liczbą, jeśli złapał, i tym, o ile chybiłeś, jeśli
        nie. Utrzymanie szczerbinki szerokiej na stopień z ćwiercią na znaku z kołyszącego się
        pokładu jest tą odrobiną opanowania, dla której ten przyrząd istnieje.</p>

        <p><b>Dziennik jest miejscem, w którym nawigator zapisuje — nie mapa.</b> Każdy
        namiar, każdy rozpoznany znak, każdy odczyt logu idzie na stronę obok papieru z
        godziną przy nim. Dwa przyciski, bo zaznaczenie na papierze i zapisanie w dzienniku to
        dwie różne czynności: ON THE CHART kładzie pozycję na papierze i jest martwy, dopóki
        dwie linie się nie przetną; IN THE LOG zapisuje linię i jest żywy zawsze.</p>

        <p><b>Używaj gumki.</b> Jedno naciśnięcie zdejmuje wątpliwą linię z papieru, a
        drugie kładzie ją tam, gdzie teraz należy — bo zwykłym powodem, dla którego pozycja
        się nie domyka, jest linia, w którą nie należało wierzyć.</p>

        <p><b>WHY</b>, na końcu rzędu przyrządów, każe każdej linii na papierze powiedzieć,
        czym jest i po co tam jest — linia pozycyjna, przeniesiona, pociągnięta ręcznie,
        odcinek z logu, zliczenie, droga nad dnem. Nakres to sześć rodzajów linii w dwóch
        kolorach bez legendy, a odróżnianie ich jest większością tej sztuki.</p>

        <div class="caveat"><b>Papier nie wie, że zrobiłeś zwrot</b><p>Zliczenie jest ciągiem
        kursów, a nie linią, a nakres jest tak stary, jak twoja ostatnia wizyta przy stole
        nawigacyjnym. Łódź doskonale wie, że zrobiła zwrot — kompas jest na grodzi — ale
        papier nie wie, więc dalej odkłada kurs, który mu ostatnio podano, z milami z logu, a
        ołówkowa pozycja odchodzi od łodzi. Dziesięć minut niezapisanego dziennika po zwrocie o
        dziewięćdziesiąt stopni to półtorej mili błędu ołówka, zmierzone. Rozłożenie mapy
        dopisuje dziennik.</p></div>""",
    ),
    (
        "racing",
        "Start",
        """<p><b>Start jest tym, co żeglarstwo ma, a czego nie ma nic innego.</b> Nie da się zatrzymać jachtu ani go zaparkować — zawsze gdzieś idzie —
        więc start wyścigu nie jest przyciskiem, tylko manewrem. Ostatnie pięć minut spędzasz
        na doprowadzeniu łodzi do linii, której nie wolno przeciąć za wcześnie, z pełną
        prędkością i na tym jej końcu, który uznałeś za uprzywilejowany. Każdy, kto się
        ścigał, wie, że tam zwykle rozstrzyga się wyścig, a każdy, kto się nie ścigał, jest
        tym zdumiony.</p>

        <p><b>Uprzywilejowany koniec jest policzony, nie wymyślony.</b> Linia długa na sto
        czterdzieści metrów, odchylona o dziesięć stopni od prostopadłej do wiatru, daje na jednym
        końcu dwadzieścia parę metrów darmowej drogi — a panel mówi, który to koniec i o ile
        metrów, bo metry są jedyną jednostką, w której da się o tym myśleć na wodzie. Za
        wcześnie za linią i wracasz, co jest tą jedną regułą, która czyni start startem.</p>

        <p><b>Cztery odcinki, pod wiatr i z wiatrem, około dwudziestu minut.</b> Trasa jest
        kładziona od wiatru w chwili jej ustawienia: jedna boja prosto na nawietrznej, druga
        prosto na zawietrznej, więc każdy odcinek jest halsowaniem albo baksztagiem i niczym
        pomiędzy. To jest celowe — na tych dwóch kursach jest co decydować. Półwiatr jest
        ślicznym kursem i nie ma na nim nic do roboty poza sterowaniem, a trasa z samych
        półwiatrów to trasa, na której wygrywa ten, kto wystartował pierwszy.</p>

        <p>Boje zalicza się bliskością — dwadzieścia pięć metrów — bo na pytanie, którą
        stroną się je minęło, ta gra nie umie odpowiedzieć uczciwie, a reguła, której nie da
        się wyegzekwować, lepiej niech zostanie żeglarzowi.</p>

        <div class="caveat"><b>Czego w tym jeszcze nie ma</b><p>Innych łodzi. Flota
        istnieje, a czasy, które zrobiłyby z tego wyścig, to dwa pola w protokole — ale nie
        ma z kim startować, więc nie ma też prawa drogi: łodzie tutaj się nie zderzają, a
        protest, którego nikt nie może złożyć, jest regułą, której nie ma. Jest zegar,
        linia, boje i odesłanie, kiedy się wyjdzie za wcześnie.</p></div>""",
    ),
    (
        "fishing",
        "Wędka",
        """<p>Do 1964 roku była otwartą łodzią rybacką, więc na lewej ćwiartce rufy stoi
        wędka w uchwycie. To nie jest minigra obok żeglowania; jest do niego przyspawana, a
        spoiną jest jeden fakt: <b>ryba na żyłce przy sześciu węzłach to ryba, którą
        stracisz.</b> Woda ciągnąca dziewięciokilową molwę bokiem z taką prędkością
        szarpie mocniej, niż żyłka wytrzyma, więc pierwszą rzeczą, kiedy coś weźmie, jest
        wytracenie prędkości — a sprzętem do tego jest szot grota, zrzut, zabackowany
        przedni żagiel i ster, który już masz. Wędka nie ma regulacji prędkości. Łódź jest
        regulacją prędkości.</p>

        <p><b>Głębokość to prędkość łodzi.</b> Przy pięciu węzłach holowana przynęta sunie
        w pierwszym metrze czy dwóch; wytrać prędkość, a opada ostro — dziewięć metrów przy
        dwóch węzłach i trzydzieści przy jednym. Dlatego makrele łowi się szybko i płytko, a
        dorsze wolno i głęboko: łódź wybiera zdobycz wcześniej niż zestaw. Plusk za rufą jest
        głębokościomierzem i znika, kiedy przynęta zejdzie na cztery metry.</p>

        <p><b>Hamulec jest całą grą i myli się w obie strony.</b> Za mocno i haczyk wychodzi
        rybie z pyska, bo to obciążenie rozpycha dziurę, w której siedzi. Za słabo i ryba w
        ogóle się nie męczy — zmęczenie liczy się z pracy wykonanej przeciw żyłce, więc
        popuszczony hamulec to ryba, której nie pokonujesz. Reguła wydrukowana na każdej
        szpuli mówi o jednej czwartej do jednej trzeciej wytrzymałości żyłki, i ten zakres jest
        namalowany na tarczy.</p>

        <p><b>Przyciśnij ją, a ucieknie.</b> Ryba, która ma jeszcze siłę, nie wisi i nie
        przyjmuje mocnego nacisku. Ciągnij mocniej, a dostaniesz dłuższą walkę — co
        jest pierwszą rzeczą, jaką każdemu mówią na łodzi.</p>

        <p><b>Nic nie przychodzi spokojnie, a ostatni ruch trzeba wyczuć w czasie.</b> Każda ryba,
        choćby pokonana, znajduje jeszcze jeden zryw na widok burty. To, co zostaje potem, nie
        jest kwestią siły, tylko wyczucia: rybę przy burcie podnosi się <em>na wznoszeniu</em>,
        pozwalając morzu podnieść ją do relingu. Przerzucona na pokład, kiedy burta opada,
        bierze cały swój ciężar na haczyk, który obluzowywał się przez pięć minut.</p>""",
    ),
    (
        "sailing-together",
        "Żeglowanie razem",
        """<p>Od dwóch do ośmiu znajomych w jednym archipelagu. Ktoś uruchamia
        przekaźnik:</p>

        <pre>sail-server --seed 42</pre>

        <p>Reszta naciska <code>Esc</code> i otwiera stronę <b>TOGETHER</b>. Gra w tej samej
        sieci jest na liście. Świat pochodzi od gospodarza — dołączasz i jesteś w jego
        archipelagu, jego pogodzie i jego porze dnia. Łódź każdego jest na mapie z nazwą
        obok.</p>

        <p>Gra na serwerze, a nie na laptopie, ma adres i <b>słowo</b>, i oba wpisuje się na
        tej stronie. <b>KNOCK</b> pyta, co tam jest, bez dołączania — jaka gra, ilu na
        pokładzie, które morze — a kiedy nic nie odpowiada, mówi, która z trzech rzeczy to
        jest: słowo, port czy serwer, który nie działa. Wszystkie trzy wyglądały kiedyś tak
        samo, a są trzema różnymi wieczorami.</p>

        <p>Przez internet działa to tak samo z wpisanym adresem gospodarza, a najmniej
        roboty daje jedna sieć Tailscale między znajomymi: bez przekierowania portów i bez
        kodu.</p>""",
    ),
    (
        "when-something-is-wrong",
        "Kiedy coś idzie nie tak",
        """<ul>
        <li><b>Nie idzie ostro.</b> Sprawdź, co jest postawione i co jest wybrane.
        Zarefowana łódź nie dojdzie tam, gdzie dojdzie pełno ożaglowana, i zacieniony wycinek
        na pierścieniu kompasu zwęża się, żeby ci to powiedzieć.</li>

        <li><b>Stoi w martwym kącie.</b> Zabackuj genuę — <code>G</code>. To jedyny żagiel,
        który da się zabackować, i to jest wyjście.</li>

        <li><b>Autopilot puścił.</b> Elektryczny mówi o tym wprost: sterownik, który przez
        sześć sekund jednocześnie siedzi na krańcówce i jest poza kursem, wie, że hamuje, a
        nie steruje. Wiatrowy nie ma czym puścić — przeciążony nie rozłącza się, tylko
        przestaje nadążać, i w końcu łódź sama robi zwrot, a urządzenie cierpliwie steruje
        nowym kursem.</li>

        <li><b>Żagle łopoczą i nic nie pomaga.</b> Masz za dużo płótna. Luzuj, potem
        refuj.</li>

        <li><b>Pozycja się nie domyka.</b> Jedna z trzech linii jest zła — najpewniej źle
        rozpoznany znak. Zetrzyj ją i weź następną.</li>

        <li><b>Coś wygląda źle, a nie umiesz tego opisać.</b> <code>F12</code> wrzuca
        zdjęcie ekranu do twojego katalogu z obrazami. Usterka, której nie da się opisać,
        jest tą, której nikt nie naprawia.</li>
        </ul>""",
    ),
]


# ---- zdjęcia, w kolejności, w jakiej opowiadają historię -------------------

SHOTS = [
    (
        "sunset.webp",
        "Koniec dnia",
        "Świt i zmierzch były dotąd tak samo szare jak południe: ocieplało się tylko "
        "światło słońca, więc po wodzie szła złota ścieżka pod niebem bez żadnej barwy. "
        "Teraz gradient idzie za słońcem — nad głową ciemnieje ku fioletowi, pali się "
        "tam, gdzie jest słońce, a po przeciwnej stronie zostaje łuk "
        "przeciwzmierzchowy — a spody chmur łapią to, co morze odbija w górę.",
    ),
    (
        "brig.webp",
        "Bryg szuka pilota",
        "Wchodzi z pełnego morza i sam do ciebie nie podejdzie. Pilotaż to prowadzenie, "
        "nie wchodzenie na pokład: doganiasz go, wychodzisz przed niego, a on od tej "
        "chwili idzie twoją wodą — w kadłubie, który ma większe zanurzenie i nie skręca "
        "tak jak twój.",
    ),
    (
        "chart.webp",
        "Stół nawigacyjny",
        "Cyrkiel, linijka, linijka równoległa, kątomierz, ołówek, stoper, log ręczny i "
        "gumka — a obok papieru dziennik pokładowy, bo mapa nie jest miejscem, w którym "
        "nawigator zapisuje. Brzeg jest odczytany z tego samego pola, na którym łódź "
        "wchodzi na mieliznę, więc nie może skłamać o głębokościach.",
    ),
    (
        "lost.webp",
        "Gdzieś na morzu, bez pojęcia gdzie",
        "Nazwy znikają ze znaków, mapa przestaje za tobą chodzić, a odzyskanie ich jest "
        "ćwiczeniem. Zobaczyć rzecz i wiedzieć, która to rzecz, to dwa różne pytania, a "
        "drugie jest całą trudnością — trzy dobrze rozstawione znaki to kształt, który "
        "pasuje do jednego miejsca na wybrzeżu i do żadnego innego.",
    ),
    (
        "squall.webp",
        "Szkwał, zapowiedziany",
        "Namiar, odległość i to, czy przechodzi bokiem, czy idzie na ciebie — na konsoli, "
        "zanim pojawi się na łodzi. Wiatr w nim jednocześnie rośnie i skręca, a szkwał "
        "zapowiedziany i zlekceważony jest tutaj najczęstszym sposobem na położenie "
        "łodzi.",
    ),
    (
        "gennaker.webp",
        "Garderoba",
        "Genua, code 0, gennaker, spinaker. Poproszenie o żagiel jest rozkazem, a nie "
        "przełącznikiem: dziób zwija stary, podpina nowy i stawia go, co jest robotą z "
        "etapami i trwa tyle, ile trwa.",
    ),
    (
        "race.webp",
        "Piętnaście sekund do strzału",
        "Nie da się zatrzymać jachtu ani go zaparkować, więc start wyścigu nie jest "
        "przyciskiem, tylko manewrem: pięć minut na odejście od linii, odczytanie wiatru, "
        "wybranie uprzywilejowanego końca i dojście pełną prędkością, ale nie za wcześnie. "
        "Panel mówi tutaj, że nie zdążysz — 233 m do linii i 124 sekundy żeglowania, żeby "
        "je pokonać — i że linia jest prosta, więc żaden koniec nie jest wart nadkładania "
        "drogi.",
    ),
    (
        "drill.webp",
        "Uczone, nie ogłaszane",
        "Ćwiczenie „człowiek za burtą” prowadzi przez ósemkę — półwiatr, zwrot, półwiatr — "
        "której uczą i RYA, i US Sailing, jedno polecenie naraz, z kursem docelowym i "
        "znakiem położonym na wodzie dla każdego odcinka. Kiedy coś trzeba zrobić w ciągu "
        "dwóch sekund, są to cztery słowa przez środek ekranu.",
    ),
    (
        "below.webp",
        "Pod powierzchnią, obok przynęty",
        "Ten projekt argumentował przeciw temu widokowi, twierdząc, że morze jest "
        "nieprzezroczyste i nie będzie co oglądać — co było argumentem o "
        "<em>powierzchni</em>. Dziesięć metrów widoczności, zielonej a nie niebieskiej, bo "
        "to morze szelfowe z wiosennym zakwitem.",
    ),
    (
        "rod.webp",
        "Wędka za rufą",
        "To nie minigra obok żeglowania. Ryba na żyłce przy sześciu węzłach to ryba, którą "
        "stracisz, więc pierwszą rzeczą, kiedy coś weźmie, jest wytracenie prędkości — "
        "szotem grota, zrzutem i sterem, które już masz.",
    ),
]

REEL_CAPTION = (
    "Trzydzieści klatek szkwału",
    "Przechyla się, morze idzie pod nią, a przyrządy odpowiadają — czyli ta połowa, "
    "której nie powie żadne zdjęcie. Nagrane przez grę fotografującą samą siebie, "
    "bo przechwytywanie ekranu wymaga uprawnienia, którego serwer budujący nie ma.",
)

HELP_SHOT_CAPTION = (
    "Ta sama lista, w programie",
    "Za znakiem w rogu, czyli pierwszą rzeczą, w którą każdy klika. Lista powyżej jest "
    "wygenerowana ze źródła tego ekranu.",
)

WIDTHS_CAPTION = (
    "720 po lewej, 960 po prawej",
    "Te same wanty, ta sama chwila, powiększone jednakowo. Na otwartej wodzie obu nie da "
    "się odróżnić; różnicę pokazuje olinowanie — czyli to samo, co gubiła najgrubsza "
    "szerokość.",
)

REEL_ALT = "Łódź żeglująca w szkwale, przechylona"
HELP_SHOT_ALT = "Ekran pomocy w grze"
WIDTHS_ALT = "To samo olinowanie narysowane w 720 i w 960 pikselach szerokości"


# ---- instalacja, czyli proza wokół dwóch poleceń --------------------------

INSTALL_MAC = """Linijka w terminalu, a nie plik do pobrania, i to celowo: macOS
kwarantannuje to, co zapisze <em>przeglądarka</em>, a nie to, co ściągnie
<code>curl</code> — a odkąd macOS 15 usunął otwieranie prawym przyciskiem, nie ma innej
prostej drogi obok komunikatu <em>&bdquo;Sail is damaged and can't be opened&rdquo;</em>
na programie, za który nikt nie zapłacił Apple'owi za poręczenie."""

INSTALL_WINDOWS = """To samo w drugą stronę: <em>&bdquo;Windows protected your
PC&rdquo;</em> bierze się ze znacznika pochodzenia z sieci, który dokłada program
pobierający — przeglądarka dokłada, <code>Invoke-WebRequest</code> nie.
<code>Sail.exe</code> ze strony wydań też działa, jeśli wolisz kliknąć <b>More info</b>
i <b>Run anyway</b>."""

IN_A_TAB = """Bez instalowania, jeśli wolisz najpierw zobaczyć: ta sama gra zbudowana
dla przeglądarki, w karcie. Zaczyna z wachtą na pokładzie i pilotem u steru, na otwartej
wodzie, żeby pierwszą minutę dało się spędzić na patrzeniu — a <b>Esc &rarr; MISSIONS
&rarr; SAILING 101</b> oprowadza po łódce, jedno urządzenie naraz. Potrzebuje WebGPU, a w praktyce
znaczy to Chrome albo Edge — Safari ma WebGPU i maluje wszystko na czarno, co jest sporem
silnika z przeglądarką, a nie usterką tej gry, i strona mówi o tym, zanim cokolwiek
pobierze. Jakichś piętnaście megabajtów po drucie, a chodzi w
niecałych pięćdziesięciu klatkach na sekundę wobec sześćdziesięciu na biurku. Tylko
samotnie: strona nie znajdzie gry, którą kolega uruchomił w sąsiednim pokoju."""

SOURCE_GAME = "gra"
SOURCE_TESTS = "zestaw testów"
SOURCE_POLAR = "polara prędkości"
