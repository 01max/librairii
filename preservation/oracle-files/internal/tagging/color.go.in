package tagging

import (
	"fmt"
	"math"
	"strconv"
	"strings"
)

const minimumTagContrast = 3.0

func normalizeAccessibleColor(value string) (string, error) {
	value = strings.ToUpper(strings.TrimSpace(value))
	if len(value) != 7 || value[0] != '#' {
		return "", fmt.Errorf("%w: color must use #RRGGBB", ErrInvalidDefinition)
	}
	channels := make([]float64, 3)
	for index := range channels {
		parsed, err := strconv.ParseUint(value[1+index*2:3+index*2], 16, 8)
		if err != nil {
			return "", fmt.Errorf("%w: color must use #RRGGBB", ErrInvalidDefinition)
		}
		channel := float64(parsed) / 255
		if channel <= 0.04045 {
			channels[index] = channel / 12.92
		} else {
			channels[index] = math.Pow((channel+0.055)/1.055, 2.4)
		}
	}
	luminance := 0.2126*channels[0] + 0.7152*channels[1] + 0.0722*channels[2]
	contrast := 1.05 / (luminance + 0.05)
	if contrast < minimumTagContrast {
		return "", fmt.Errorf(
			"%w: color contrast %.2f is below %.1f:1",
			ErrInvalidDefinition,
			contrast,
			minimumTagContrast,
		)
	}
	return value, nil
}
