import collections
import statistics
import sys
from PIL import Image


def blue(pixel):
    r, g, b = pixel[:3]
    return b > 110 and b > r + 35 and b > g + 20


def green(pixel):
    r, g, b = pixel[:3]
    return g > 110 and g > r + 40 and g > b + 40


def load(path):
    image = Image.open(path).convert('RGB')
    return image, image.load()


def run_from(pixels, y, start, limit, predicate):
    run = 0
    for x in range(start, limit):
        if predicate(pixels[x, y]):
            run += 1
        else:
            break
    return run


def longest_run(pixels, y, start, limit, predicate):
    best = (0, -1)
    x = start
    while x < limit:
        if predicate(pixels[x, y]):
            run = run_from(pixels, y, x, limit, predicate)
            if run > best[0]:
                best = (run, x)
            x += run
        else:
            x += 1
    return best


def bar(path, width, top=80, bottom=240):
    image, pixels = load(path)
    limit = min(width, image.width)
    best = (0, -1, -1)
    for y in range(top, min(bottom, image.height)):
        run, start = longest_run(pixels, y, 0, limit, blue)
        if run > best[0]:
            best = (run, y, start)
    run, y, start = best
    if run == 0:
        return 0, -1, -1, -1
    below = run_from(pixels, y + 1, start, limit, blue) if y + 1 < image.height else 0
    above = run_from(pixels, y - 1, start, limit, blue) if y > 0 else 0
    if below >= above:
        return run, y, y + 1, start
    return run, y - 1, y, start


def fill_at(path, width, y_top, y_bottom, start):
    image, pixels = load(path)
    limit = min(width, image.width)
    return max(run_from(pixels, y, start, limit, blue) for y in range(y_top, y_bottom + 1))


def track(path, width, y_bottom, start):
    image, pixels = load(path)
    limit = min(width, image.width)
    reference = y_bottom - 4
    samples = [pixels[x, reference] for x in range(start, limit)]
    background = tuple(statistics.median(channel[i] for channel in samples) for i in range(3))

    def lighter(pixel):
        return all(pixel[i] > background[i] + 2 for i in range(3)) and not blue(pixel)

    return run_from(pixels, y_bottom, start, limit, lighter)


def count(path, width, y, predicate, rows=1):
    image, pixels = load(path)
    total = 0
    for row in range(max(0, y - rows), min(image.height, y + rows + 1)):
        for x in range(0, min(width, image.width)):
            if predicate(pixels[x, row]):
                total += 1
    return total


def drives_bar(path, y, left, right):
    image, pixels = load(path)
    fills = []
    tracks = []
    for row in range(max(0, y - 10), min(image.height, y + 11)):
        background = collections.Counter(pixels[x, row] for x in range(left, right)).most_common(1)[0][0]

        def differs(pixel, limit):
            return max(abs(pixel[i] - background[i]) for i in range(3)) > limit

        fills.append(longest_run(pixels, row, left, right, lambda pixel: differs(pixel, 25))[0])
        tracks.append(longest_run(pixels, row, left, right, lambda pixel: differs(pixel, 4))[0])
    best = (0, 0)
    for index in range(len(fills) - 3):
        window = fills[index:index + 4]
        low = min(window)
        if low >= 2 and max(window) - low <= 1 and low > best[0]:
            best = (low, max(tracks[index:index + 4]))
    return best


command = sys.argv[1]
if command == 'bar':
    print(*bar(sys.argv[2], int(sys.argv[3])))
elif command == 'fill-at':
    print(fill_at(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]), int(sys.argv[6])))
elif command == 'track':
    print(track(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])))
elif command == 'blue-count':
    print(count(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), blue))
elif command == 'green-count':
    print(count(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), green))
elif command == 'drives-bar':
    print(*drives_bar(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])))
